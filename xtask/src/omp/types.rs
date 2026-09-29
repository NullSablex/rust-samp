//! From C++ types to Rust ones.
//!
//! A type is read from libclang's structure — typedef names, template
//! arguments, the canonical record — never from its spelling. What cannot be
//! stated with certainty maps to `None`, and the caller writes down why.

use std::fmt;

use clang::{EntityKind, TemplateArgument, Type, TypeKind};

use super::generator::Generator;

/// A Rust type the generated code names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Rust {
    /// A primitive: `i32`, `bool`, `usize`, ...
    Prim(&'static str),
    /// A named type: a value type the SDK writes by hand, a mirrored struct,
    /// a generated union, an interface handle.
    Named(String),
    /// A raw pointer.
    Ptr { mutable: bool, to: Box<Rust> },
    /// `std::ffi::c_void`, behind a pointer.
    Void,
    /// `[T; N]`.
    Array(Box<Rust>, u64),
    /// `Pair<A, B>`.
    Pair(Box<Rust>, Box<Rust>),
    /// `Span<T>`.
    Span(Box<Rust>),
    /// `HybridString<N>`.
    Hybrid(u64),
    /// `FlatSet<T>`, `None` for an item the SDK has no handle for.
    FlatSet(Option<String>),
}

impl fmt::Display for Rust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rust::Prim(name) => f.write_str(name),
            Rust::Named(name) => f.write_str(name),
            Rust::Ptr { mutable, to } => {
                write!(f, "{} {to}", if *mutable { "*mut" } else { "*const" })
            }
            Rust::Void => f.write_str("std::ffi::c_void"),
            Rust::Array(item, len) => write!(f, "[{item}; {len}]"),
            Rust::Pair(first, second) => write!(f, "Pair<{first}, {second}>"),
            Rust::Span(item) => write!(f, "Span<{item}>"),
            Rust::Hybrid(len) => write!(f, "HybridString<{len}>"),
            Rust::FlatSet(Some(item)) => write!(f, "FlatSet<{item}>"),
            Rust::FlatSet(None) => f.write_str("FlatSet<std::ffi::c_void>"),
        }
    }
}

impl Rust {
    pub fn ptr(mutable: bool, to: Rust) -> Rust {
        Rust::Ptr {
            mutable,
            to: Box::new(to),
        }
    }

    /// A type Rust names with generic arguments.
    pub fn is_generic(&self) -> bool {
        matches!(
            self,
            Rust::Pair(..) | Rust::Span(_) | Rust::Hybrid(_) | Rust::FlatSet(_)
        )
    }

    pub fn is_hybrid(&self) -> bool {
        matches!(self, Rust::Hybrid(_))
    }
}

/// C++ integer and floating-point types by the name a typedef gives them.
/// These win over the canonical type: `size_t` is `usize` on every target,
/// though it is an `unsigned int` on i686.
fn named_integer(name: &str) -> Option<&'static str> {
    Some(match name {
        "size_t" => "usize",
        "UID" | "uint64_t" => "u64",
        "int64_t" => "i64",
        "int32_t" => "i32",
        "uint32_t" => "u32",
        "int16_t" => "i16",
        "uint16_t" => "u16",
        "int8_t" => "i8",
        "uint8_t" => "u8",
        _ => return None,
    })
}

/// A builtin type, by kind. `long` is 32 bits on i686 under both ABIs.
fn builtin(kind: TypeKind) -> Option<&'static str> {
    Some(match kind {
        TypeKind::Bool => "bool",
        TypeKind::CharS | TypeKind::SChar => "i8",
        TypeKind::CharU | TypeKind::UChar => "u8",
        TypeKind::Short => "i16",
        TypeKind::UShort => "u16",
        TypeKind::Int | TypeKind::Long => "i32",
        TypeKind::UInt | TypeKind::ULong => "u32",
        TypeKind::LongLong => "i64",
        TypeKind::ULongLong => "u64",
        TypeKind::Float => "f32",
        TypeKind::Double => "f64",
        _ => return None,
    })
}

/// Value types the SDK writes by hand, with their size — what decides how a
/// method returns one — and the neutral value a getter answers with when the
/// object is not there.
pub fn hand_value(name: &str) -> Option<(usize, Option<&'static str>)> {
    Some(match name {
        "Colour" => (4, None),
        // `std::chrono` durations and time points are a class holding the
        // count; minutes and hours are 4 bytes under MSVC, within the rule.
        "Vector2" | "StringView" | "Milliseconds" | "Seconds" | "Minutes" | "Hours"
        | "Microseconds" | "TimePoint" | "WorldTimePoint" => (8, None),
        "Vector3" => (12, Some("Vector3::ZERO")),
        "Vector4" => (16, Some("Vector4::ZERO")),
        "GangZonePos" | "GTAQuat" => (16, None),
        "SemanticVersion" => (6, None),
        _ => return None,
    })
}

/// The name a type goes by at this level of sugar: a typedef's, or a record's.
fn sugar_name(ty: Type<'_>) -> Option<String> {
    let declaration = ty.get_declaration()?;
    match declaration.get_kind() {
        EntityKind::TypedefDecl | EntityKind::TypeAliasDecl => declaration.get_name(),
        _ => None,
    }
}

/// One step down the sugar: through an elaborated name or a typedef.
fn desugar(ty: Type<'_>) -> Option<Type<'_>> {
    match ty.get_kind() {
        TypeKind::Elaborated => ty.get_elaborated_type(),
        TypeKind::Typedef => ty.get_declaration()?.get_typedef_underlying_type(),
        _ => None,
    }
}

/// The non-type template argument at `index` of a class template
/// specialization: `N` in `std::array<T, N>` or `HybridString<N>`.
fn integral_argument(ty: Type<'_>, index: usize) -> Option<u64> {
    let arguments = ty
        .get_canonical_type()
        .get_declaration()?
        .get_template_arguments()?;
    match arguments.get(index)? {
        TemplateArgument::Integral(signed, _) => u64::try_from(*signed).ok(),
        _ => None,
    }
}

/// The declaration's name and its enclosing namespace, for telling
/// `std::pair` from a type that happens to be called `pair`.
fn record_identity(ty: Type<'_>) -> Option<(String, String)> {
    let declaration = ty.get_canonical_type().get_declaration()?;
    let name = declaration.get_name()?;
    let namespace = declaration
        .get_semantic_parent()
        .and_then(|p| p.get_name())
        .unwrap_or_default();
    Some((namespace, name))
}

impl<'tu> Generator<'tu> {
    /// The Rust type for a C++ type held by value, or `None`.
    pub fn rust_of(&mut self, ty: Type<'tu>) -> Option<Rust> {
        // Names first: a typedef may say more than the type it stands for.
        let mut level = Some(ty);
        while let Some(current) = level {
            if let Some(name) = sugar_name(current) {
                if let Some(prim) = named_integer(&name) {
                    return Some(Rust::Prim(prim));
                }
                if hand_value(&name).is_some() || self.is_mirrored(&name) {
                    return Some(Rust::Named(name));
                }
            }
            if let Some(found) = self.template_of(current) {
                return Some(found);
            }
            level = desugar(current);
        }

        let canonical = ty.get_canonical_type();
        if let Some(prim) = builtin(canonical.get_kind()) {
            return Some(Rust::Prim(prim));
        }
        match canonical.get_kind() {
            TypeKind::Enum => {
                // Without a fixed type, GCC picks `unsigned int` for an enum with
                // no negative value and MSVC always `int`: the same width, and
                // `int` is what both accept. MSVC's reading says which it is.
                let declaration = canonical.get_declaration()?;
                let twin = self
                    .msvc
                    .get(&super::ast::qualified_name(declaration))
                    .unwrap_or(declaration);
                let underlying = twin.get_enum_underlying_type()?;
                builtin(underlying.get_canonical_type().get_kind()).map(Rust::Prim)
            }
            TypeKind::Pointer => Some(self.pointer_to(canonical.get_pointee_type()?, false)),
            TypeKind::ConstantArray => {
                let item = self.rust_of(canonical.get_element_type()?)?;
                Some(Rust::Array(
                    Box::new(item),
                    u64::try_from(canonical.get_size()?).ok()?,
                ))
            }
            TypeKind::Record => {
                let declaration = canonical.get_declaration()?;
                let name = declaration.get_name()?;
                if hand_value(&name).is_some() {
                    return Some(Rust::Named(name));
                }
                // A template the containers above did not recognise is not
                // something to mirror: its layout depends on its arguments.
                if declaration.get_template().is_some() {
                    return None;
                }
                self.mirror(declaration).then_some(Rust::Named(name))
            }
            _ => None,
        }
    }

    /// The containers the headers name through templates: `Pair`, `Span`,
    /// `StaticArray`, `HybridString`, and the `robin_hood` sets.
    fn template_of(&mut self, ty: Type<'tu>) -> Option<Rust> {
        let (namespace, name) = record_identity(ty)?;
        let arguments = || ty.get_template_argument_types();
        match (namespace.as_str(), name.as_str()) {
            ("std", "pair") => {
                let types = arguments()?;
                let first = self.rust_of((*types.first()?)?)?;
                let second = self.rust_of((*types.get(1)?)?)?;
                Some(Rust::Pair(Box::new(first), Box::new(second)))
            }
            ("span_lite", "span") => {
                let item = self.rust_of((*arguments()?.first()?)?)?;
                Some(Rust::Span(Box::new(item)))
            }
            ("std", "array") => {
                let item = self.rust_of((*arguments()?.first()?)?)?;
                Some(Rust::Array(Box::new(item), integral_argument(ty, 1)?))
            }
            (_, "HybridString") => Some(Rust::Hybrid(integral_argument(ty, 0)?)),
            ("detail", "Table") => {
                // `FlatPtrHashSet<T>` and `FlatHashSet<T*>` are both a table
                // of `T*`: the key is the third argument.
                let canonical = ty.get_canonical_type();
                let key = canonical.get_template_argument_types()?.get(2).copied()??;
                let item = key
                    .get_pointee_type()?
                    .get_canonical_type()
                    .get_declaration()?
                    .get_name()?;
                let known = self.handles.contains(&item) || self.hand.types.contains(&item);
                Some(Rust::FlatSet(known.then_some(item)))
            }
            _ => None,
        }
    }

    /// A pointer to `target`: a handle, a type the SDK can state, or `c_void`.
    pub fn pointer_to(&mut self, target: Type<'tu>, constant: bool) -> Rust {
        let mutable = !constant;
        if target.get_canonical_type().get_kind() == TypeKind::Pointer {
            let inner = target
                .get_canonical_type()
                .get_pointee_type()
                .map(|t| self.pointer_to(t, false));
            return Rust::ptr(mutable, inner.unwrap_or(Rust::Void));
        }
        if let Some(name) = record_name(target) {
            if self.handles.contains(&name) {
                return Rust::ptr(true, Rust::Named(name));
            }
            if self.hand.types.contains(&name) {
                return Rust::ptr(mutable, Rust::Named(name));
            }
        }
        match self.rust_of(target) {
            Some(rust) => Rust::ptr(mutable, rust),
            None => Rust::ptr(mutable, Rust::Void),
        }
    }
}

/// The name of the record or typedef a type names directly.
pub fn record_name(ty: Type<'_>) -> Option<String> {
    let mut level = Some(ty);
    while let Some(current) = level {
        if let Some(declaration) = current.get_declaration()
            && matches!(
                declaration.get_kind(),
                EntityKind::StructDecl
                    | EntityKind::ClassDecl
                    | EntityKind::TypedefDecl
                    | EntityKind::TypeAliasDecl
            )
        {
            return declaration.get_name();
        }
        level = desugar(current);
    }
    None
}

/// Whether a type is spelled with a leading `const`.
pub fn is_const(ty: Type<'_>) -> bool {
    ty.is_const_qualified()
}

/// The type a reference or pointer refers to.
pub fn pointee(ty: Type<'_>) -> Option<Type<'_>> {
    match ty.get_kind() {
        TypeKind::LValueReference | TypeKind::RValueReference | TypeKind::Pointer => {
            ty.get_pointee_type()
        }
        _ => None,
    }
}

pub fn is_reference(ty: Type<'_>) -> bool {
    matches!(
        ty.get_kind(),
        TypeKind::LValueReference | TypeKind::RValueReference
    )
}
