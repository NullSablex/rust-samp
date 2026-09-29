//! The state the generation shares across modules, and the rules for a
//! method's parameters and result.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use clang::{Entity, Type, TypeKind};

use super::ast::Records;
use super::hand::Hand;
use super::handlers::Handler;
use super::mirror::{Mirrored, Union};
use super::types::{Rust, hand_value, is_const, is_reference, pointee, record_name};

/// Why a method, struct or handler is left out. Written into the output.
#[derive(Debug)]
pub struct Skip(pub String);

impl Skip {
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

/// How a wrapper calls the method and hands the result back.
pub enum Returns {
    /// Nothing comes back.
    Nothing,
    /// A value the declared return type carries: a primitive, a pointer, a
    /// struct larger than eight bytes. `neutral` answers for a null object.
    Plain { rust: Rust, neutral: String },
    /// A struct of at most eight bytes, or a generic one: MSVC returns it
    /// through a hidden pointer, which `call_vtable_small_struct!` handles.
    Small(Rust),
}

pub struct Generator<'tu> {
    pub records: Records<'tu>,
    pub msvc: Records<'tu>,
    pub include_dir: PathBuf,
    /// What the hand-written SDK already defines.
    pub hand: Hand,
    /// Every interface handle, hand-written or generated.
    pub handles: HashSet<String>,
    /// Function names taken, hand-written or generated so far.
    pub taken: HashSet<String>,
    /// Constants and `ComponentInterface` impls defined so far.
    pub consts: HashSet<String>,
    /// Handles declared so far: a module declares its handle only if none has.
    pub declared: HashSet<String>,
    pub mirrored: BTreeMap<String, Mirrored>,
    pub unions: BTreeMap<String, Union>,
    pub not_mirrored: BTreeMap<String, String>,
    pub handlers: BTreeMap<String, Handler>,
    pub not_handlers: BTreeMap<String, String>,
}

impl<'tu> Generator<'tu> {
    /// A generator over `records`, knowing what the SDK writes by hand and
    /// every handle the spec declares.
    pub fn new(
        records: Records<'tu>,
        msvc: Records<'tu>,
        include_dir: PathBuf,
        hand: Hand,
        spec: &[super::spec::Interface],
    ) -> Self {
        let handles = hand
            .handles
            .iter()
            .cloned()
            .chain(spec.iter().map(|i| i.handle.clone()))
            .collect();
        Self {
            records,
            msvc,
            include_dir,
            taken: hand.functions.clone(),
            consts: hand.consts.clone(),
            declared: hand.handles.clone(),
            handles,
            hand,
            mirrored: BTreeMap::new(),
            unions: BTreeMap::new(),
            not_mirrored: BTreeMap::new(),
            handlers: BTreeMap::new(),
            not_handlers: BTreeMap::new(),
        }
    }

    pub fn is_mirrored(&self, name: &str) -> bool {
        self.mirrored.contains_key(name)
    }

    /// Whether the C++ side copies it with a constructor: such a type travels
    /// by reference only, since the ABIs pass it by value differently.
    pub fn is_ref_only(&self, rust: &Rust) -> bool {
        match rust {
            Rust::Named(name) => self.mirrored.get(name).is_some_and(|m| m.ref_only),
            Rust::Hybrid(_) => true,
            _ => false,
        }
    }

    /// Size in bytes under MSVC, for the value types the SDK knows.
    fn size_of(&self, rust: &Rust) -> Option<usize> {
        let Rust::Named(name) = rust else { return None };
        hand_value(name)
            .map(|(size, _)| size)
            .or_else(|| self.mirrored.get(name).map(|m| m.msvc.size / 8))
    }

    /// The generated handler for an interface the server calls back through —
    /// `HTTPResponseHandler`, `OptionEnumeratorCallback` — or `None`.
    fn callback_object(&mut self, target: Type<'tu>) -> Option<String> {
        let declaration = target.get_canonical_type().get_declaration()?;
        let name = declaration.get_name()?;
        if self.handles.contains(&name) {
            return None;
        }
        let record = self.records.get(&name)?;
        if !super::mirror::is_interface(record) {
            return None;
        }
        self.handler_of(&name, None)
    }

    fn by_value(&mut self, ty: Type<'tu>, spelled: &str, returns: bool) -> Result<Rust, Skip> {
        let verb = if returns { "returns" } else { "takes" };
        let rust = self
            .rust_of(ty)
            .ok_or_else(|| Skip::new(format!("{verb} `{spelled}`")))?;
        if self.is_ref_only(&rust) {
            let how = if returns { "return" } else { "pass" };
            return Err(Skip::new(format!(
                "{verb} `{spelled}` by value, which the two ABIs {how} differently"
            )));
        }
        Ok(rust)
    }

    /// The Rust type of a parameter.
    pub fn param_type(&mut self, ty: Type<'tu>) -> Result<String, Skip> {
        let spelled = ty.get_display_name();
        if let Some(target) = pointee(ty) {
            if is_reference(ty) && target.get_canonical_type().get_kind() == TypeKind::Pointer {
                // `const T *&`: the callee writes a pointer back.
                let inner = target
                    .get_canonical_type()
                    .get_pointee_type()
                    .map(|t| self.pointer_to(t, is_const(t)));
                return Ok(format!("*mut {}", inner.unwrap_or(Rust::Void)));
            }
            if let Some(name) = record_name(target)
                && self.handles.contains(&name)
            {
                return Ok(format!("*mut {name}"));
            }
            if let Some(handler) = self.callback_object(target) {
                return Ok(format!("*mut {handler}"));
            }
            if is_reference(ty) {
                let rust = self.rust_of(target).ok_or_else(|| {
                    Skip::new(format!("takes `{spelled}`, which the SDK does not mirror"))
                })?;
                // `const Vector3 &` is a pointer to one the callee only reads;
                // a Rust reference is that pointer. Without `const` the callee
                // writes through it: an out-parameter.
                return Ok(if is_const(target) {
                    format!("&{rust}")
                } else {
                    format!("&mut {rust}")
                });
            }
            return Ok(self.pointer_to(target, is_const(target)).to_string());
        }
        self.by_value(ty, &spelled, false)
            .map(|rust| rust.to_string())
    }

    /// How the method's result comes back. `chain` holds the records whose
    /// references a chaining setter returns.
    pub fn returns(&mut self, ty: Type<'tu>, chain: &[Entity<'tu>]) -> Result<Returns, Skip> {
        let spelled = ty.get_display_name();
        if ty.get_kind() == TypeKind::Void {
            return Ok(Returns::Nothing);
        }
        if let Some(target) = pointee(ty) {
            let name = record_name(target);
            // A setter returning the object itself, for chaining: the caller
            // already holds it.
            if is_reference(ty)
                && name
                    .as_deref()
                    .is_some_and(|n| chain.iter().any(|c| c.get_name().as_deref() == Some(n)))
            {
                return Ok(Returns::Nothing);
            }
            if let Some(dispatcher) = self.dispatcher(target)? {
                return Ok(dispatcher);
            }
            let constant = is_const(target);
            let null = if constant {
                "std::ptr::null()"
            } else {
                "std::ptr::null_mut()"
            };
            if let Some(name) = &name {
                if self.handles.contains(name) {
                    return Ok(Returns::Plain {
                        rust: Rust::ptr(true, Rust::Named(name.clone())),
                        neutral: "std::ptr::null_mut()".into(),
                    });
                }
                if is_reference(ty) && self.hand.types.contains(name) {
                    return Ok(Returns::Plain {
                        rust: Rust::ptr(!constant, Rust::Named(name.clone())),
                        neutral: null.into(),
                    });
                }
            }
            if is_reference(ty) {
                // A reference into the server's own copy: the pointer, to read
                // in place.
                let rust = self.rust_of(target).ok_or_else(|| {
                    Skip::new(format!(
                        "returns `{spelled}`, which the SDK does not mirror"
                    ))
                })?;
                return Ok(Returns::Plain {
                    rust: Rust::ptr(!constant, rust),
                    neutral: null.into(),
                });
            }
            let rust = self.pointer_to(target, constant);
            let null = match &rust {
                Rust::Ptr { mutable: false, .. } => "std::ptr::null()",
                _ => "std::ptr::null_mut()",
            };
            return Ok(Returns::Plain {
                rust,
                neutral: null.into(),
            });
        }
        let rust = self.by_value(ty, &spelled, true)?;
        if let Rust::Prim(prim) = rust {
            let neutral = match prim {
                "bool" => "false",
                "f32" | "f64" => "0.0",
                _ => "0",
            };
            return Ok(Returns::Plain {
                rust,
                neutral: neutral.into(),
            });
        }
        if let Some(size) = self.size_of(&rust)
            && size > 8
        {
            let neutral = match &rust {
                Rust::Named(name) => hand_value(name)
                    .and_then(|(_, neutral)| neutral)
                    .map(str::to_owned),
                _ => None,
            }
            .unwrap_or_else(|| self.default_of(&rust));
            return Ok(Returns::Plain { rust, neutral });
        }
        Ok(Returns::Small(rust))
    }

    /// `IEventDispatcher<H> &`: the generic handle, typed by its handler.
    fn dispatcher(&mut self, target: Type<'tu>) -> Result<Option<Returns>, Skip> {
        let Some(declaration) = target.get_canonical_type().get_declaration() else {
            return Ok(None);
        };
        if declaration.get_name().as_deref() != Some("IEventDispatcher") {
            return Ok(None);
        }
        let argument = target
            .get_template_argument_types()
            .and_then(|args| args.first().copied().flatten())
            .ok_or_else(|| Skip::new(format!("returns `{}`", target.get_display_name())))?;
        let handler_record = argument.get_canonical_type().get_declaration();
        let handler_name = handler_record
            .and_then(|d| d.get_name())
            .unwrap_or_default();
        // A template handler (`PoolEventHandler<IPlayer>`) carries its argument.
        let template_argument = argument
            .get_template_argument_types()
            .and_then(|args| args.first().copied().flatten())
            .and_then(|t| record_name(t));
        let key = match &template_argument {
            Some(arg) => format!("{handler_name}<{arg}>"),
            None => handler_name.clone(),
        };
        let Some(handler) = self.handler_of(&handler_name, template_argument.as_deref()) else {
            let why = self.not_handlers.get(&key).cloned().unwrap_or_default();
            return Err(Skip::new(format!(
                "returns the dispatcher of `{key}`, which {why}"
            )));
        };
        if self.hand.types.contains(&handler) {
            return Err(Skip::new(format!(
                "`{handler}` and its dispatcher are written by hand"
            )));
        }
        Ok(Some(Returns::Plain {
            rust: Rust::ptr(true, Rust::Named(format!("EventDispatcher<{handler}>"))),
            neutral: "std::ptr::null_mut()".into(),
        }))
    }

    /// An expression for a value of `rust` to stand in when there is none.
    pub fn default_of(&self, rust: &Rust) -> String {
        match rust {
            Rust::Named(name) if self.mirrored.get(name).is_some_and(|m| m.default.is_none()) => {
                // Every mirrored field is an integer, a float, a bool, a pointer
                // or a struct of them: all zero is a valid value.
                format!("unsafe {{ std::mem::zeroed::<{rust}>() }}")
            }
            // Pointers have no `Default`, and all zero is valid for every
            // generic the headers use.
            _ if rust.is_generic() => format!("unsafe {{ std::mem::zeroed::<{rust}>() }}"),
            Rust::Named(name) if name == "SemanticVersion" => {
                "SemanticVersion::new(0, 0, 0)".into()
            }
            _ => format!("{rust}::default()"),
        }
    }
}
