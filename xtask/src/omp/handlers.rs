//! Handlers: the interfaces the server calls back through, written as vtables
//! of plain functions for the plugin to fill.

use clang::{Entity, EntityKind, Type, TypeKind};

use super::ast::{constant, header_of};
use super::generator::Generator;
use super::mirror::snake;
use super::types::{Rust, is_const, is_reference, pointee, record_name};

/// A handler written into `handlers.rs`.
pub struct Handler {
    /// The Rust type: `ObjectHandler`, `PlayerPoolHandler`.
    pub rust: String,
    pub header: String,
    pub entries: Vec<Entry>,
}

/// One method of a handler's vtable.
pub struct Entry {
    /// The C++ signature, for the doc comment.
    pub signature: String,
    pub field: String,
    pub args: Vec<String>,
    pub ret: Option<&'static str>,
    /// What the C++ body does, as a Rust expression; `None` for a pure method.
    pub default: Option<String>,
}

/// Narrow integers a callback receives: C++ callers need not extend them to a
/// word, and a Rust `bool` or `u8` argument assumes they did. The whole word
/// comes in, and the value is its low bits — the rule `VirtualReturn` applies
/// to return values.
fn widened(prim: &'static str) -> &'static str {
    match prim {
        "bool" | "u8" | "u16" => "u32",
        "i8" | "i16" => "i32",
        other => other,
    }
}

/// The template parameter a type is, when it is one (`T` in `T &`).
fn is_template_parameter(ty: Type<'_>) -> bool {
    matches!(ty.get_canonical_type().get_kind(), TypeKind::Unexposed)
        || ty
            .get_declaration()
            .is_some_and(|d| d.get_kind() == EntityKind::TemplateTypeParameter)
}

impl<'tu> Generator<'tu> {
    /// The Rust handler type for `cpp` (`ObjectEventHandler` -> `ObjectHandler`,
    /// `PoolEventHandler<IPlayer>` -> `PlayerPoolHandler`), written into
    /// `handlers.rs` unless the SDK has it by hand; `None` when the handler
    /// cannot be stated with certainty, the reason under its key in
    /// `not_handlers`.
    pub fn handler_of(&mut self, cpp: &str, argument: Option<&str>) -> Option<String> {
        let key = match argument {
            Some(arg) => format!("{cpp}<{arg}>"),
            None => cpp.to_owned(),
        };
        let rust = match argument {
            Some(arg) if cpp == "PoolEventHandler" => {
                format!("{}PoolHandler", arg.strip_prefix('I').unwrap_or(arg))
            }
            Some(_) => {
                self.not_handlers
                    .insert(key, "is a template the generator does not know".into());
                return None;
            }
            None => cpp
                .strip_suffix("EventHandler")
                .map_or_else(|| cpp.to_owned(), |base| format!("{base}Handler")),
        };
        if self.hand.types.contains(&rust) || self.handlers.contains_key(&key) {
            return Some(rust);
        }
        if self.not_handlers.contains_key(&key) {
            return None;
        }
        let Some(record) = self.records.get(cpp) else {
            self.not_handlers
                .insert(key, "is not declared where it is used".into());
            return None;
        };
        self.not_handlers
            .insert(key.clone(), "refers to itself".into());
        match self.handler_entries(record, argument) {
            Ok(entries) => {
                let header = header_of(record, &self.include_dir).unwrap_or_default();
                self.not_handlers.remove(&key);
                self.handlers.insert(
                    key,
                    Handler {
                        rust: rust.clone(),
                        header,
                        entries,
                    },
                );
                Some(rust)
            }
            Err(why) => {
                self.not_handlers.insert(key, why);
                None
            }
        }
    }

    fn handler_entries(
        &mut self,
        record: Entity<'tu>,
        argument: Option<&str>,
    ) -> Result<Vec<Entry>, String> {
        if !super::ast::bases(record).is_empty() {
            return Err("has a base class".into());
        }
        let children = record.get_children();
        if children
            .iter()
            .any(|c| c.get_kind() == EntityKind::Destructor && c.is_virtual_method())
        {
            return Err("has a virtual destructor".into());
        }
        let methods: Vec<Entity<'tu>> = children
            .into_iter()
            .filter(|m| m.get_kind() == EntityKind::Method && m.is_virtual_method())
            .collect();
        let mut names: Vec<String> = methods.iter().filter_map(clang::Entity::get_name).collect();
        names.sort();
        names.dedup();
        if names.len() != methods.len() {
            return Err("overloads a method, which MSVC reorders".into());
        }

        let mut entries = Vec::new();
        for method in methods {
            let name = method.get_name().unwrap_or_default();
            let params = method.get_arguments().unwrap_or_default();
            let mut spelled = Vec::new();
            let mut args = Vec::new();
            for param in params {
                let ty = param
                    .get_type()
                    .ok_or_else(|| format!("`{name}` has a parameter without a type"))?;
                let substituted =
                    argument.filter(|_| pointee(ty).is_some_and(is_template_parameter));
                spelled.push(match substituted {
                    Some(arg) => format!("{arg} &"),
                    None => ty.get_display_name(),
                });
                let arg = match substituted {
                    Some(arg) => Some(format!("*mut {arg}")),
                    None => self.callback_arg(ty),
                };
                args.push(
                    arg.ok_or_else(|| format!("`{name}` takes a value the SDK does not mirror"))?,
                );
            }
            let result = method
                .get_result_type()
                .ok_or_else(|| format!("`{name}` has no result type"))?;
            let ret_spelled = result.get_display_name();
            let ret = match result.get_canonical_type().get_kind() {
                TypeKind::Void => None,
                TypeKind::Enum => return Err(format!("`{name}` returns `{ret_spelled}`")),
                _ => match self.rust_of(result) {
                    Some(Rust::Prim(prim)) => Some(prim),
                    _ => return Err(format!("`{name}` returns `{ret_spelled}`")),
                },
            };
            let default = if method.is_pure_virtual_method() {
                None
            } else {
                let not_literal = || format!("`{name}` has a body that is not a plain literal");
                Some(match (ret, body(method).ok_or_else(not_literal)?) {
                    (None, Body::Empty) => "()".to_owned(),
                    (Some("bool"), Body::Returns(v)) => (v != 0).to_string(),
                    (Some("f32" | "f64"), Body::Returns(v)) => format!("{v}.0"),
                    (Some(_), Body::Returns(v)) => v.to_string(),
                    _ => return Err(not_literal()),
                })
            };
            entries.push(Entry {
                signature: format!("{ret_spelled} {name}({})", spelled.join(", ")),
                field: snake(&name),
                args,
                ret,
                default,
            });
        }
        if entries.is_empty() {
            return Err("declares no methods".into());
        }
        if entries.iter().any(|e| e.default.is_none())
            && entries.iter().any(|e| e.default.is_some())
        {
            return Err("mixes pure and defined methods".into());
        }
        Ok(entries)
    }

    /// The Rust type of an argument the server passes to a handler.
    fn callback_arg(&mut self, ty: Type<'tu>) -> Option<String> {
        if let Some(target) = pointee(ty) {
            if let Some(name) = record_name(target)
                && self.handles.contains(&name)
            {
                return Some(format!("*mut {name}"));
            }
            if is_reference(ty) && target.get_canonical_type().get_kind() == TypeKind::Pointer {
                let inner = target.get_canonical_type().get_pointee_type()?;
                return Some(format!("*mut {}", self.pointer_to(inner, is_const(target))));
            }
            return Some(self.pointer_to(target, is_const(target)).to_string());
        }
        let rust = self.rust_of(ty)?;
        if self.is_ref_only(&rust) {
            return None;
        }
        Some(match rust {
            Rust::Prim(prim) => widened(prim).to_owned(),
            other => other.to_string(),
        })
    }
}

/// What a handler method's inline body does.
enum Body {
    /// `{ }`
    Empty,
    /// `{ return <constant>; }`
    Returns(i64),
}

/// The body, when it is one of the shapes a `DEFAULT` can reproduce.
fn body(method: Entity<'_>) -> Option<Body> {
    let body = method
        .get_children()
        .into_iter()
        .find(|c| c.get_kind() == EntityKind::CompoundStmt)?;
    match body.get_children().as_slice() {
        [] => Some(Body::Empty),
        [statement] if statement.get_kind() == EntityKind::ReturnStmt => statement
            .get_children()
            .first()
            .copied()
            .and_then(constant)
            .map(Body::Returns),
        _ => None,
    }
}
