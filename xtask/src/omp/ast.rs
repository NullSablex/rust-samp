//! The headers as libclang reads them: one translation unit per ABI holding
//! every header the spec names, and an index of their records.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use clang::{Entity, EntityKind, EntityVisitResult, EvaluationResult, Index, TranslationUnit};

use super::sdk::{Abi, Sdk};

/// A translation unit including every header in `headers`, parsed for `abi`.
pub fn parse<'i>(
    index: &'i Index<'_>,
    sdk: &Sdk,
    abi: Abi,
    source: &Path,
) -> Result<TranslationUnit<'i>> {
    let mut args = vec![
        "-std=c++17".to_owned(),
        format!("--target={}", abi.triple()),
        "-w".to_owned(),
    ];
    args.extend(sdk.flags(abi));
    let unit = index
        .parser(source)
        .arguments(&args)
        .parse()
        .with_context(|| format!("libclang could not parse the headers for {}", abi.triple()))?;
    let errors: Vec<String> = unit
        .get_diagnostics()
        .into_iter()
        .filter(|d| d.get_severity() >= clang::diagnostic::Severity::Error)
        .map(|d| d.get_text())
        .collect();
    if !errors.is_empty() {
        bail!(
            "the headers do not compile for {}:\n  {}",
            abi.triple(),
            errors.join("\n  ")
        );
    }
    Ok(unit)
}

/// Every record defined in a translation unit, by name.
///
/// A name maps to its first definition in source order, which is what a
/// header-local lookup would find. Nested records are also filed under their
/// qualified name (`PeerNetworkData::NetworkID`), which is how the MSVC side
/// is matched to the Itanium one.
pub struct Records<'tu> {
    by_name: HashMap<String, Entity<'tu>>,
}

impl<'tu> Records<'tu> {
    pub fn index(unit: &'tu TranslationUnit<'_>) -> Self {
        let mut by_name = HashMap::new();
        unit.get_entity().visit_children(|entity, _| {
            // Class templates too: a handler can be one (`PoolEventHandler<T>`).
            // Enums too, so each ABI's reading of one can be compared.
            let indexed = is_record(entity)
                || matches!(
                    entity.get_kind(),
                    EntityKind::ClassTemplate | EntityKind::EnumDecl
                );
            if indexed && entity.is_definition() && !entity.is_anonymous() {
                if let Some(name) = entity.get_name() {
                    by_name.entry(name).or_insert(entity);
                }
                by_name.entry(qualified_name(entity)).or_insert(entity);
            }
            EntityVisitResult::Recurse
        });
        Self { by_name }
    }

    pub fn get(&self, name: &str) -> Option<Entity<'tu>> {
        self.by_name.get(name).copied()
    }
}

pub fn is_record(entity: Entity<'_>) -> bool {
    matches!(
        entity.get_kind(),
        EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
    )
}

/// `Outer::Inner` for a nested record, the name alone at namespace scope —
/// namespaces left out, as the headers spell the types.
pub fn qualified_name(entity: Entity<'_>) -> String {
    let mut parts = vec![entity.get_name().unwrap_or_default()];
    let mut parent = entity.get_semantic_parent();
    while let Some(outer) = parent {
        if !is_record(outer) {
            break;
        }
        parts.push(outer.get_name().unwrap_or_default());
        parent = outer.get_semantic_parent();
    }
    parts.reverse();
    parts.join("::")
}

/// The header a declaration sits in, relative to the SDK's include directory.
pub fn header_of(entity: Entity<'_>, include_dir: &Path) -> Option<String> {
    let file = entity.get_location()?.get_file_location().file?;
    let path = file.get_path();
    let relative = path.strip_prefix(include_dir).ok()?;
    Some(relative.to_string_lossy().into_owned())
}

/// A method as a wrapper sees it.
pub struct Method<'tu> {
    pub name: String,
    pub result: clang::Type<'tu>,
    pub params: Vec<(String, clang::Type<'tu>)>,
    pub variadic: bool,
}

/// The pure virtual methods `record` declares itself, in declaration order.
pub fn pure_methods(record: Entity<'_>) -> Vec<Method<'_>> {
    record
        .get_children()
        .into_iter()
        .filter(|m| m.get_kind() == EntityKind::Method && m.is_pure_virtual_method())
        .filter_map(|m| {
            let params = m
                .get_arguments()?
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    Some((
                        p.get_name().unwrap_or_else(|| format!("arg{i}")),
                        p.get_type()?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Method {
                name: m.get_name()?,
                result: m.get_result_type()?,
                params,
                variadic: m.is_variadic(),
            })
        })
        .collect()
}

/// The records a class's first base chain passes through — the ones laid out
/// at offset 0, whose methods share its primary vtable — the class first.
pub fn primary_chain<'tu>(records: &Records<'tu>, leaf: &str) -> Vec<Entity<'tu>> {
    let mut chain = Vec::new();
    let mut current = records.get(leaf);
    while let Some(record) = current {
        chain.push(record);
        current = bases(record).first().and_then(|base| {
            // A template base (`IPoolComponent<IActor>`) is looked up by its
            // template's name, as the chain only needs to know what it is.
            let declaration = base.get_type()?.get_declaration()?;
            records.get(&declaration.get_name()?)
        });
    }
    chain
}

pub fn bases(record: Entity<'_>) -> Vec<Entity<'_>> {
    record
        .get_children()
        .into_iter()
        .filter(|c| c.get_kind() == EntityKind::BaseSpecifier)
        .collect()
}

/// How an interface identifies itself: `PROVIDE_UID` for a component,
/// `PROVIDE_EXT_UID` for a per-player extension.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UidKind {
    Component,
    Extension,
}

pub struct Uid {
    pub kind: UidKind,
    /// The header's constant the macro was given (`ObjectsComponent_UID`).
    pub constant: String,
    pub value: u64,
}

/// The UID `record` provides, read from the constant the macro declares in it:
/// `IID` for `PROVIDE_UID`, `ExtensionIID` for `PROVIDE_EXT_UID`.
pub fn uid_of(record: Entity<'_>) -> Option<Uid> {
    record.get_children().into_iter().find_map(|member| {
        if member.get_kind() != EntityKind::VarDecl {
            return None;
        }
        let kind = match member.get_name()?.as_str() {
            "IID" => UidKind::Component,
            "ExtensionIID" => UidKind::Extension,
            _ => return None,
        };
        let value = match member.evaluate()? {
            EvaluationResult::UnsignedInteger(v) => v,
            EvaluationResult::SignedInteger(v) => u64::from_ne_bytes(v.to_ne_bytes()),
            _ => return None,
        };
        let constant = member.get_children().into_iter().find_map(|init| {
            (init.get_kind() != EntityKind::TypeRef)
                .then(|| init.get_name())
                .flatten()
        })?;
        Some(Uid {
            kind,
            constant,
            value,
        })
    })
}

/// An integer or boolean an expression evaluates to, if it is a constant.
pub fn constant(expr: Entity<'_>) -> Option<i64> {
    match expr.evaluate()? {
        EvaluationResult::SignedInteger(v) => Some(v),
        EvaluationResult::UnsignedInteger(v) => i64::try_from(v).ok(),
        _ => None,
    }
}
