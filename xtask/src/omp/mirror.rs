//! Structs the headers pass by value or by reference, mirrored as Rust ones.

use std::collections::HashMap;

use clang::{Accessibility, Entity, EntityKind, Type, TypeKind};

use super::ast::{self, constant, header_of, qualified_name};
use super::generator::Generator;
use super::types::Rust;

/// A struct's size and its fields' offsets for one ABI, in bits.
pub struct Layout {
    pub size: usize,
    /// One per field, `None` where it does not start on a byte (a bit-field
    /// run that begins mid-byte).
    pub offsets: Vec<Option<usize>>,
}

/// A field of a mirrored struct.
pub struct Field {
    /// The C++ name, or `(anonymous union)`.
    pub cpp: String,
    /// The Rust name.
    pub rust: String,
    pub ty: Rust,
    /// A run of bit-fields collected into this integer: name and width each.
    pub bits: Option<Vec<(String, u64)>>,
    /// The C++ member whose offset is this field's: itself, the first of a
    /// bit-field run, or the first member of an anonymous union.
    offset_of: String,
}

pub struct Mirrored {
    /// The header declaring it, relative to the SDK's include directory.
    pub header: String,
    pub fields: Vec<Field>,
    /// Each field's C++ default, when every one is a known constant.
    pub default: Option<Vec<String>>,
    /// Copied with a constructor on the C++ side: passed by reference only.
    pub ref_only: bool,
    pub itanium: Layout,
    pub msvc: Layout,
}

/// An anonymous union — or an anonymous struct inside one.
pub struct Union {
    pub is_union: bool,
    /// C++ name, Rust name, type.
    pub members: Vec<(String, String, Rust)>,
}

/// A member of a record, in the order a mirror reads them.
enum Member<'tu> {
    Field(Entity<'tu>),
    /// An anonymous union or struct: its members belong to the parent.
    Anonymous(Entity<'tu>),
}

fn members(record: Entity<'_>) -> Result<Vec<Member<'_>>, String> {
    let mut out = Vec::new();
    for child in record.get_children() {
        if child.get_accessibility() == Some(Accessibility::Private)
            || child.get_accessibility() == Some(Accessibility::Protected)
        {
            if child.get_kind() == EntityKind::FieldDecl {
                return Err("has a private member".into());
            }
            continue;
        }
        match child.get_kind() {
            EntityKind::FieldDecl => out.push(Member::Field(child)),
            EntityKind::StructDecl | EntityKind::UnionDecl if child.is_anonymous_record_decl() => {
                out.push(Member::Anonymous(child));
            }
            EntityKind::Method if child.is_virtual_method() => {
                return Err("has virtual methods".into());
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The type of an unnamed union or struct a named field is declared with
/// (`union { ... } v6;`), or `None` for any other field.
fn unnamed_record(field: Entity<'_>) -> Option<Entity<'_>> {
    let declaration = field.get_type()?.get_canonical_type().get_declaration()?;
    (declaration.is_anonymous() && ast::is_record(declaration)).then_some(declaration)
}

impl<'tu> Generator<'tu> {
    /// Mirrors `record` if every field is something the SDK can state with
    /// certainty; whether it is mirrored now.
    pub fn mirror(&mut self, record: Entity<'tu>) -> bool {
        let Some(name) = record.get_name() else {
            return false;
        };
        if self.mirrored.contains_key(&name) {
            return true;
        }
        if self.not_mirrored.contains_key(&name) || self.hand.types.contains(&name) {
            return false;
        }
        if !matches!(
            record.get_kind(),
            EntityKind::StructDecl | EntityKind::ClassDecl
        ) {
            return false;
        }
        let Some(record_type) = record.get_type() else {
            return false;
        };
        if is_interface(record) {
            return false; // held by pointer
        }

        self.not_mirrored
            .insert(name.clone(), "refers to itself".into());
        let fields = match self.mirror_fields(&name, record) {
            Ok(fields) => fields,
            Err(why) => {
                self.not_mirrored.insert(name, why);
                return false;
            }
        };
        let (Some(itanium), Some(msvc)) = (
            layout(record_type, &fields),
            self.msvc_layout(record, &fields),
        ) else {
            self.not_mirrored
                .insert(name, "clang gave no layout for it".into());
            return false;
        };
        let default = self.defaults(record, &fields);
        let header = header_of(record, &self.include_dir).unwrap_or_default();
        let ref_only = !is_trivially_copyable(record);
        self.not_mirrored.remove(&name);
        self.mirrored.insert(
            name,
            Mirrored {
                header,
                fields,
                default,
                ref_only,
                itanium,
                msvc,
            },
        );
        true
    }

    fn mirror_fields(&mut self, name: &str, record: Entity<'tu>) -> Result<Vec<Field>, String> {
        if !ast::bases(record).is_empty() {
            return Err("has a base class".into());
        }
        let mut fields: Vec<Field> = Vec::new();
        let mut run: Option<(Rust, Vec<(String, u64)>)> = None;
        let close = |fields: &mut Vec<Field>, run: &mut Option<(Rust, Vec<(String, u64)>)>| {
            if let Some((storage, bits)) = run.take() {
                let first = bits[0].0.clone();
                fields.push(Field {
                    cpp: first.clone(),
                    rust: format!("bits_{}", snake(&first)),
                    ty: storage,
                    offset_of: first,
                    bits: Some(bits),
                });
            }
        };

        for member in members(record)? {
            let field = match member {
                Member::Anonymous(inner) => {
                    close(&mut fields, &mut run);
                    let union = self.mirror_union(&format!("{name}{}", fields.len()), inner)?;
                    let first = self
                        .union_anchor(&union)
                        .ok_or("has an anonymous union without a named member")?;
                    fields.push(Field {
                        cpp: "(anonymous union)".into(),
                        rust: format!("u{}", fields.len()),
                        ty: Rust::Named(union),
                        bits: None,
                        offset_of: first,
                    });
                    continue;
                }
                Member::Field(field) => field,
            };
            let field_name = field.get_name().unwrap_or_default();
            if field.is_bit_field() {
                let width = field
                    .get_bit_field_width()
                    .and_then(|w| u64::try_from(w).ok());
                let storage =
                    field
                        .get_type()
                        .and_then(|t| self.rust_of(t))
                        .map(|rust| match rust {
                            Rust::Prim("bool") => Rust::Prim("u8"),
                            other => other,
                        });
                let (
                    Some(width),
                    Some(storage @ Rust::Prim("u8" | "u16" | "u32" | "i8" | "i16" | "i32")),
                ) = (width, storage)
                else {
                    let spelled = field
                        .get_type()
                        .map(|t| t.get_display_name())
                        .unwrap_or_default();
                    return Err(format!("has a bit-field `{field_name}` of `{spelled}`"));
                };
                if run.as_ref().is_some_and(|(current, _)| *current != storage) {
                    close(&mut fields, &mut run);
                }
                run.get_or_insert_with(|| (storage, Vec::new()))
                    .1
                    .push((field_name, width));
                continue;
            }
            close(&mut fields, &mut run);
            if let Some(unnamed) = unnamed_record(field) {
                // A named field of an unnamed union or struct type.
                let union =
                    self.mirror_union(&format!("{name}{}", pascal(&snake(&field_name))), unnamed)?;
                fields.push(Field {
                    cpp: field_name.clone(),
                    rust: snake(&field_name),
                    ty: Rust::Named(union),
                    bits: None,
                    offset_of: field_name,
                });
                continue;
            }
            let ty = field.get_type().ok_or("has a field without a type")?;
            let rust = self
                .rust_of(ty)
                .ok_or_else(|| format!("has a `{}` member", ty.get_display_name()))?;
            fields.push(Field {
                cpp: field_name.clone(),
                rust: snake(&field_name),
                ty: rust,
                bits: None,
                offset_of: field_name,
            });
        }
        close(&mut fields, &mut run);
        if fields.is_empty() {
            return Err("has no fields".into());
        }
        Ok(fields)
    }

    /// A Rust type for an anonymous union or struct; its name.
    ///
    /// A member that cannot be stated is left out of a union — the other
    /// members give it its size, and the enclosing struct's layout assertions
    /// check that. A struct inside is all or nothing.
    fn mirror_union(&mut self, name: &str, record: Entity<'tu>) -> Result<String, String> {
        let is_union = record.get_kind() == EntityKind::UnionDecl;
        let mut members = Vec::new();
        for member in record.get_children() {
            match member.get_kind() {
                EntityKind::StructDecl | EntityKind::UnionDecl
                    if member.is_anonymous_record_decl() =>
                {
                    match self.mirror_union(&format!("{name}_{}", members.len()), member) {
                        Ok(inner) => members.push((
                            "(anonymous)".to_owned(),
                            format!("s{}", members.len()),
                            Rust::Named(inner),
                        )),
                        Err(_) if is_union => {}
                        Err(why) => return Err(why),
                    }
                }
                EntityKind::FieldDecl => {
                    let field_name = member.get_name().unwrap_or_default();
                    if member.is_bit_field() {
                        if is_union {
                            continue;
                        }
                        return Err("has bit-fields inside an anonymous struct".into());
                    }
                    if let Some(unnamed) = unnamed_record(member) {
                        match self.mirror_union(&format!("{name}_{}", snake(&field_name)), unnamed)
                        {
                            Ok(inner) => members.push((
                                field_name.clone(),
                                snake(&field_name),
                                Rust::Named(inner),
                            )),
                            Err(_) if is_union => {}
                            Err(why) => return Err(why),
                        }
                        continue;
                    }
                    let Some(ty) = member.get_type() else {
                        continue;
                    };
                    match self.rust_of(ty) {
                        Some(rust) if !self.is_ref_only(&rust) => {
                            members.push((field_name.clone(), snake(&field_name), rust));
                        }
                        _ if is_union => {}
                        _ => return Err(format!("has a `{}` member", ty.get_display_name())),
                    }
                }
                _ => {}
            }
        }
        if members.is_empty() {
            return Err("has an anonymous union with nothing the SDK can state".into());
        }
        self.unions
            .insert(name.to_owned(), Union { is_union, members });
        Ok(name.to_owned())
    }

    /// The first named member of a generated union, looking through the
    /// anonymous structs inside it: where the union starts.
    fn union_anchor(&self, union: &str) -> Option<String> {
        let (cpp, _, ty) = self.unions.get(union)?.members.first()?;
        match ty {
            Rust::Named(inner) if cpp == "(anonymous)" => self.union_anchor(inner),
            _ => Some(cpp.clone()),
        }
    }

    /// The same record's layout under MSVC, found by its qualified name.
    fn msvc_layout(&self, record: Entity<'tu>, fields: &[Field]) -> Option<Layout> {
        let twin = self.msvc.get(&qualified_name(record))?;
        layout(twin.get_type()?, fields)
    }

    /// Each field's C++ default — an in-class initializer, or one from a
    /// constructor without parameters — as long as every one is a known
    /// constant; zero for a field with none.
    fn defaults(&self, record: Entity<'tu>, fields: &[Field]) -> Option<Vec<String>> {
        // In-class initializers first; a constructor without parameters wins
        // over them, wherever it is declared.
        let mut given: HashMap<String, Option<i64>> = HashMap::new();
        let mut from_constructor: HashMap<String, Option<i64>> = HashMap::new();
        for child in record.get_children() {
            match child.get_kind() {
                EntityKind::FieldDecl => {
                    // The in-class initializer is the field's one expression child.
                    let init = child
                        .get_children()
                        .into_iter()
                        .find(clang::Entity::is_expression);
                    if let Some(init) = init {
                        given.insert(child.get_name().unwrap_or_default(), constant(init));
                    }
                }
                EntityKind::Constructor if child.get_arguments().is_some_and(|a| a.is_empty()) => {
                    let children = child.get_children();
                    for pair in children.windows(2) {
                        if pair[0].get_kind() == EntityKind::MemberRef {
                            from_constructor
                                .insert(pair[0].get_name().unwrap_or_default(), constant(pair[1]));
                        }
                    }
                }
                _ => {}
            }
        }
        given.extend(from_constructor);
        fields
            .iter()
            .map(|field| {
                if field.bits.is_some() || self.unions.contains_key(&field.ty.to_string()) {
                    return None;
                }
                match given.get(&field.cpp) {
                    Some(None) => None,
                    Some(Some(0)) | None => self.zero_of(&field.ty),
                    Some(Some(value)) => Some(match field.ty {
                        Rust::Prim("bool") => (*value != 0).to_string(),
                        Rust::Prim("f32" | "f64") => format!("{value}.0"),
                        _ => value.to_string(),
                    }),
                }
            })
            .collect()
    }

    /// A zero of `rust`, when it has one to write.
    pub fn zero_of(&self, rust: &Rust) -> Option<String> {
        Some(match rust {
            Rust::Prim("bool") => "false".into(),
            Rust::Prim("f32" | "f64") => "0.0".into(),
            Rust::Prim(_) => "0".into(),
            Rust::Named(name) => match name.as_str() {
                "Vector2" | "Colour" => format!("{name}::default()"),
                "Vector3" | "Vector4" => format!("{name}::ZERO"),
                "GTAQuat" => "GTAQuat::IDENTITY".into(),
                "Milliseconds" | "Seconds" | "Minutes" | "Hours" | "Microseconds" | "TimePoint"
                | "WorldTimePoint" => {
                    format!("{name}(0)")
                }
                _ if self.mirrored.get(name).is_some_and(|m| m.default.is_some()) => {
                    format!("{name}::default()")
                }
                _ => return None,
            },
            Rust::Array(item, len) => format!("[{}; {len}]", self.zero_of(item)?),
            _ => return None,
        })
    }
}

/// A struct's size and its fields' offsets, from libclang.
fn layout(ty: Type<'_>, fields: &[Field]) -> Option<Layout> {
    let size = ty.get_sizeof().ok()?.checked_mul(8)?;
    let offsets = fields
        .iter()
        .map(|f| {
            ty.get_offsetof(&f.offset_of)
                .ok()
                .map(|bit| (bit % 8 == 0).then_some(bit))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Layout { size, offsets })
}

/// An interface: a record with virtual methods, held by pointer.
pub fn is_interface(record: Entity<'_>) -> bool {
    record
        .get_children()
        .iter()
        .any(|m| m.get_kind() == EntityKind::Method && m.is_virtual_method())
        || ast::bases(record).iter().any(|b| {
            b.get_type()
                .and_then(|t| t.get_canonical_type().get_declaration())
                .is_some_and(is_interface)
        })
}

/// Copyable byte for byte: no user-provided copy or move constructor or
/// destructor in it, in any member, or in any array's element.
fn is_trivially_copyable(record: Entity<'_>) -> bool {
    // An implicit specialization (`HybridString<16>`) shows libclang no
    // members of its own; its template declares them.
    let record = record
        .get_template()
        .filter(|_| record.get_children().is_empty())
        .unwrap_or(record);
    let own = record
        .get_children()
        .iter()
        .all(|child| match child.get_kind() {
            EntityKind::Constructor => {
                !(child.is_copy_constructor() || child.is_move_constructor())
                    || child.is_defaulted()
            }
            EntityKind::Destructor => child.is_defaulted(),
            _ => true,
        });
    own && record
        .get_children()
        .iter()
        .filter(|c| c.get_kind() == EntityKind::FieldDecl)
        .all(|field| field.get_type().is_none_or(type_is_trivially_copyable))
}

fn type_is_trivially_copyable(ty: Type<'_>) -> bool {
    let canonical = ty.get_canonical_type();
    match canonical.get_kind() {
        TypeKind::ConstantArray => canonical
            .get_element_type()
            .is_none_or(type_is_trivially_copyable),
        TypeKind::Record => canonical
            .get_declaration()
            .is_none_or(is_trivially_copyable),
        _ => true,
    }
}

/// `getHealth` -> `get_health`, `NPCComponent_UID` -> `npc_component_uid`.
///
/// A run of capitals is one word (`NPC`, `IP`), ending where a lowercase letter
/// starts the next one.
pub fn snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            if prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    let mut collapsed = String::with_capacity(out.len());
    for c in out.chars() {
        if !(c == '_' && collapsed.ends_with('_')) {
            collapsed.push(c);
        }
    }
    collapsed
}

/// `v6` -> `V6`, `ipv6_address` -> `Ipv6Address`.
fn pascal(snake: &str) -> String {
    snake
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_keeps_acronyms_whole() {
        assert_eq!(snake("getHealth"), "get_health");
        assert_eq!(snake("NPCComponent_UID"), "npc_component_uid");
        assert_eq!(snake("isUsingOmp"), "is_using_omp");
        assert_eq!(snake("VehicleID"), "vehicle_id");
        assert_eq!(snake("v6"), "v6");
        assert_eq!(snake("getIP"), "get_ip");
    }

    #[test]
    fn pascal_case_joins_words() {
        assert_eq!(pascal("v6"), "V6");
        assert_eq!(pascal("ipv6_address"), "Ipv6Address");
    }
}
