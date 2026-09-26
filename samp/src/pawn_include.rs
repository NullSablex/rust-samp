//! Comparing a hand-written Pawn include against the natives a plugin registers.
//!
//! `#[native]` derives a declaration for every native, and
//! [`crate::plugin::pawn_include`] writes them out as an `.inc`. That works when
//! the include is generated. Plugins that maintain theirs by hand do so for
//! reasons the Rust signature cannot express — default values, `sizeof(dest)`,
//! varargs, documentation — and pay for it with drift: a native renamed in Rust
//! and forgotten in the include fails only when a script calls it.
//!
//! This module finds that drift. It parses declarations out of an include and
//! compares them with what the plugin registered, reporting what disagrees and
//! staying quiet about what a hand-written include is entitled to add.
//!
//! ```rust,no_run
//! for finding in samp::pawn_include::compare_file("email_samp.inc").unwrap() {
//!     log::warn!("{finding}");
//! }
//! ```
//!
//! Setting `SAMP_PAWN_INCLUDE_CHECK` to a path makes the SDK run the comparison
//! at load and log whatever it finds, which is the shape a CI job wants.
//!
//! ## What is compared, and what is not
//!
//! The name, the return tag, the number of arguments, and each argument's tag,
//! by-reference marker and array marker. A hand-written include may add default
//! values (`account = 0`), size expressions (`sizeof(dest)`), varargs
//! (`{Float,_}:...`) and any amount of documentation: those are additions the
//! Rust side has no way to state, so they are not divergences.
//!
//! An argument list ending in varargs also stops arity checking there — the
//! declaration is deliberately open-ended.

use std::fmt;
use std::path::Path;

/// One argument of a declaration, reduced to what both sides can state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// `Float:` and friends, lowercased without the colon; empty when untagged.
    pub tag: String,
    /// Declared as `&arg`.
    pub by_reference: bool,
    /// Declared as `arg[]`.
    pub array: bool,
}

/// A `native` declaration, from either side of the comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    /// Return tag, lowercased without the colon; empty when untagged.
    pub tag: String,
    pub arguments: Vec<Argument>,
    /// The list ends in `...`, so the argument count is open.
    pub variadic: bool,
    /// The native this one is an alias of: `native Email_Close(...) = email_close;`
    /// declares the name a script calls, implemented by the registered native
    /// after the `=`. Comparison follows the `=`, so an include that renames the
    /// whole surface is not reported as drift.
    pub implemented_by: Option<String>,
    /// Whether the argument list is known at all. A `raw` native registers its
    /// name but parses its own arguments, so nothing but the name can be
    /// compared; the include is the only place its shape is written down.
    pub shape_known: bool,
}

/// Something the include and the plugin disagree about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Divergence {
    /// Registered by the plugin, absent from the include — a script calling it
    /// will not compile.
    MissingFromInclude { native: String },
    /// Declared in the include, not registered — a script calling it compiles
    /// and fails at runtime.
    NotRegistered { native: String },
    /// Both sides have it, with a different number of arguments.
    Arity {
        native: String,
        include: usize,
        plugin: usize,
    },
    /// Both sides have it, with a different return tag.
    ReturnTag {
        native: String,
        include: String,
        plugin: String,
    },
    /// One argument is declared differently: a tag, a `&`, or a `[]`.
    ArgumentShape {
        native: String,
        position: usize,
        include: Argument,
        plugin: Argument,
    },
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn describe(argument: &Argument) -> String {
            let mut text = String::new();
            if argument.by_reference {
                text.push('&');
            }
            if !argument.tag.is_empty() {
                text.push_str(&argument.tag);
                text.push(':');
            }
            text.push_str("arg");
            if argument.array {
                text.push_str("[]");
            }
            text
        }

        match self {
            Self::MissingFromInclude { native } => write!(
                f,
                "{native} is registered but missing from the include; a script calling it will not compile"
            ),
            Self::NotRegistered { native } => write!(
                f,
                "{native} is declared in the include but not registered; a script calling it fails at runtime"
            ),
            Self::Arity {
                native,
                include,
                plugin,
            } => write!(
                f,
                "{native} takes {plugin} argument(s), the include declares {include}"
            ),
            Self::ReturnTag {
                native,
                include,
                plugin,
            } => {
                let shown = |tag: &str| {
                    if tag.is_empty() {
                        "untagged".into()
                    } else {
                        format!("{tag}:")
                    }
                };
                write!(
                    f,
                    "{native} returns {}, the include declares {}",
                    shown(plugin),
                    shown(include)
                )
            }
            Self::ArgumentShape {
                native,
                position,
                include,
                plugin,
            } => write!(
                f,
                "{native} argument {position} is {}, the include declares {}",
                describe(plugin),
                describe(include)
            ),
        }
    }
}

/// Strips `//` and `/* */` comments, so a commented-out declaration is not read
/// as one.
fn without_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_block = false;

    while let Some(c) = chars.next() {
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                in_block = true;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Splits an argument list at top-level commas, ignoring those inside braces or
/// parentheses — `{Float,_}:...` and `sizeof(a,b)` are single arguments.
fn split_arguments(list: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for c in list.chars() {
        match c {
            '{' | '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Reads one argument, dropping what only a hand-written include can say.
fn parse_argument(text: &str) -> Option<Argument> {
    // A default value is the include's business, not the plugin's.
    let text = text.split('=').next().unwrap_or(text).trim();
    let text = text.trim_start_matches("const").trim();
    // `...`, alone or tagged as `{Float,_}:...`, marks varargs, not an argument.
    if text.is_empty() || text.contains("...") {
        return None;
    }

    let by_reference = text.starts_with('&');
    let text = text.trim_start_matches('&').trim();

    let (tag, rest) = match text.split_once(':') {
        // `{Float,_}:...` is the varargs form, which carries no single tag.
        Some((tag, rest)) if !tag.starts_with('{') => (tag.trim().to_lowercase(), rest),
        _ => (String::new(), text),
    };

    Some(Argument {
        tag,
        by_reference,
        array: rest.contains('['),
    })
}

/// Every `native` declaration in `source`.
///
/// Understands what a hand-written include contains: comments, default values,
/// size expressions, varargs, and declarations spread over several lines.
#[must_use]
pub fn parse(source: &str) -> Vec<Declaration> {
    let cleaned = without_comments(source);
    let mut declarations = Vec::new();
    let mut rest = cleaned.as_str();

    while let Some(start) = rest.find("native ") {
        rest = &rest[start + "native ".len()..];
        let Some(open) = rest.find('(') else { break };
        let Some(close) = rest.find(')') else { break };
        if close < open {
            continue;
        }

        let head = rest[..open].trim();
        let (tag, name) = match head.rsplit_once(':') {
            Some((tag, name)) => (tag.trim().to_lowercase(), name.trim()),
            None => (String::new(), head),
        };
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            rest = &rest[close..];
            continue;
        }

        // `native Alias(...) = real_name;` — everything up to the `;` after the
        // argument list, which is where an alias is written.
        let tail = &rest[close + 1..];
        let tail = &tail[..tail.find(';').unwrap_or(0)];
        let tail_len = tail.len();
        let implemented_by = tail
            .split_once('=')
            .map(|(_, target)| target.trim().to_string())
            .filter(|target| {
                !target.is_empty() && target.chars().all(|c| c.is_alphanumeric() || c == '_')
            });

        let list = &rest[open + 1..close];
        let variadic = list.contains("...");
        let arguments = split_arguments(list)
            .iter()
            .filter_map(|argument| parse_argument(argument))
            .collect();

        declarations.push(Declaration {
            name: name.to_string(),
            tag,
            arguments,
            variadic,
            implemented_by,
            shape_known: true,
        });
        rest = &rest[close + 1 + tail_len..];
    }
    declarations
}

/// Compares an include's text with the natives this plugin registered.
///
/// Findings come back in a stable order: what is missing, what is extra, then
/// the mismatches, each sorted by native name, so a CI job's output does not
/// churn between runs.
#[must_use]
pub fn compare(include_source: &str) -> Vec<Divergence> {
    compare_declarations(&parse(include_source), &registered_declarations())
}

/// What the plugin registered, as declarations.
///
/// `#[native]` renders a `raw` native commented out, since its arity is not in
/// the Rust signature. The name is still registered, so it is read back here and
/// marked [`Declaration::shape_known`] `false` — its presence is compared, its
/// shape is not.
fn registered_declarations() -> Vec<Declaration> {
    crate::plugin::native_decls()
        .iter()
        .filter_map(|decl| {
            let (source, shape_known) = match decl.trim_start().strip_prefix("//") {
                Some(rest) => (rest.trim_start(), false),
                None => (*decl, true),
            };
            let mut parsed = parse(source).into_iter().next()?;
            parsed.shape_known = shape_known;
            Some(parsed)
        })
        .collect()
}

/// Reads `path` and compares it with the registered natives.
///
/// # Errors
/// Propagates the [`std::io::Error`] when the file cannot be read.
pub fn compare_file(path: impl AsRef<Path>) -> std::io::Result<Vec<Divergence>> {
    Ok(compare(&std::fs::read_to_string(path)?))
}

/// The comparison itself, over two parsed lists — the part worth testing
/// without a plugin behind it.
#[must_use]
pub fn compare_declarations(
    declared: &[Declaration],
    registered: &[Declaration],
) -> Vec<Divergence> {
    // What the plugin registered is the name after an `=`, when there is one,
    // and the declared name otherwise.
    fn implementing(declaration: &Declaration) -> &str {
        declaration
            .implemented_by
            .as_deref()
            .unwrap_or(&declaration.name)
    }

    let mut findings = Vec::new();

    let mut missing: Vec<&Declaration> = registered
        .iter()
        .filter(|r| !declared.iter().any(|d| implementing(d) == r.name))
        .collect();
    missing.sort_by(|a, b| a.name.cmp(&b.name));
    findings.extend(missing.into_iter().map(|r| Divergence::MissingFromInclude {
        native: r.name.clone(),
    }));

    let mut extra: Vec<&Declaration> = declared
        .iter()
        .filter(|d| !registered.iter().any(|r| r.name == implementing(d)))
        .collect();
    extra.sort_by(|a, b| a.name.cmp(&b.name));
    findings.extend(extra.into_iter().map(|d| Divergence::NotRegistered {
        native: d.name.clone(),
    }));

    let mut shared: Vec<(&Declaration, &Declaration)> = declared
        .iter()
        .filter_map(|d| {
            registered
                .iter()
                .find(|r| r.name == implementing(d))
                .map(|r| (d, r))
        })
        .collect();
    shared.sort_by(|a, b| a.0.name.cmp(&b.0.name));

    for (include, plugin) in shared {
        // A raw native wrote down neither its arguments nor, reliably, more
        // than its name: only the include states its shape.
        if !plugin.shape_known {
            continue;
        }
        if include.tag != plugin.tag {
            findings.push(Divergence::ReturnTag {
                native: include.name.clone(),
                include: include.tag.clone(),
                plugin: plugin.tag.clone(),
            });
        }

        // A variadic declaration is open-ended on purpose, so only the
        // arguments it does name are compared.
        if !include.variadic && include.arguments.len() != plugin.arguments.len() {
            findings.push(Divergence::Arity {
                native: include.name.clone(),
                include: include.arguments.len(),
                plugin: plugin.arguments.len(),
            });
        }

        for (position, (declared_arg, registered_arg)) in include
            .arguments
            .iter()
            .zip(plugin.arguments.iter())
            .enumerate()
        {
            if declared_arg != registered_arg {
                findings.push(Divergence::ArgumentShape {
                    native: include.name.clone(),
                    position: position + 1,
                    include: declared_arg.clone(),
                    plugin: registered_arg.clone(),
                });
            }
        }
    }
    findings
}

/// Runs the comparison when `SAMP_PAWN_INCLUDE_CHECK` names an include.
///
/// Called by the SDK right after `on_load`, not from the entry point: the check
/// has nothing but log output to show, and under native open.mp a component's
/// entry point runs before any logger exists, so anything logged there is lost.
///
/// Anything found is logged as a warning; a missing or unreadable file is
/// reported and otherwise ignored, since a development check must never take a
/// server down.
pub(crate) fn check_if_requested() {
    let Some(path) = std::env::var_os("SAMP_PAWN_INCLUDE_CHECK") else {
        return;
    };
    let shown = path.to_string_lossy().into_owned();

    match compare_file(&path) {
        Ok(findings) if findings.is_empty() => {
            crate::macros::sdk_info!("{shown} matches the registered natives");
        }
        Ok(findings) => {
            crate::macros::sdk_warn!("{shown} disagrees with the registered natives:");
            for finding in findings {
                crate::macros::sdk_warn!("  {finding}");
            }
        }
        Err(e) => crate::macros::sdk_warn!("could not read {shown}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argument(tag: &str, by_reference: bool, array: bool) -> Argument {
        Argument {
            tag: tag.to_string(),
            by_reference,
            array,
        }
    }

    #[test]
    fn reads_a_plain_declaration() {
        let declarations = parse("native bool:Counter_Get(&out);");
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0].name, "Counter_Get");
        assert_eq!(declarations[0].tag, "bool");
        assert_eq!(declarations[0].arguments, vec![argument("", true, false)]);
    }

    #[test]
    fn ignores_what_only_an_include_can_say() {
        // Defaults, `sizeof`, `const` and documentation are the include's to
        // add; none of them is a divergence.
        let declarations = parse(
            "/* docs */ native bool:email_status(account = 0, dest[], dest_len = sizeof(dest));",
        );
        assert_eq!(declarations.len(), 1);
        assert_eq!(
            declarations[0].arguments,
            vec![
                argument("", false, false),
                argument("", false, true),
                argument("", false, false),
            ]
        );
    }

    #[test]
    fn a_commented_out_declaration_is_not_one() {
        let source = "// native Old_Removed(a);\n/* native Also_Gone(); */\nnative Live(a);";
        let names: Vec<_> = parse(source).into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["Live"]);
    }

    #[test]
    fn varargs_leave_the_argument_count_open() {
        let declarations =
            parse("native bool:email_test(account = 0, const format[], {Float,_}:...);");
        assert!(declarations[0].variadic);
        // The varargs marker itself is not an argument.
        assert_eq!(declarations[0].arguments.len(), 2);

        let registered = parse("native bool:email_test(account, const format[], extra1, extra2);");
        // More arguments than the include names is fine: it said "and more".
        assert!(compare_declarations(&declarations, &registered).is_empty());
    }

    #[test]
    fn reports_a_native_the_include_forgot() {
        let findings = compare_declarations(&parse(""), &parse("native Foo(a);"));
        assert_eq!(
            findings,
            vec![Divergence::MissingFromInclude {
                native: "Foo".into()
            }]
        );
        assert!(findings[0].to_string().contains("will not compile"));
    }

    #[test]
    fn reports_a_declaration_with_no_native_behind_it() {
        let findings = compare_declarations(&parse("native Ghost(a);"), &parse(""));
        assert_eq!(
            findings,
            vec![Divergence::NotRegistered {
                native: "Ghost".into()
            }]
        );
        assert!(findings[0].to_string().contains("fails at runtime"));
    }

    #[test]
    fn reports_arity_and_tag_and_shape() {
        let declared = parse("native Foo(a, b);\nnative bool:Bar(x);\nnative Baz(n);");
        let registered = parse("native Foo(a);\nnative Bar(x);\nnative Baz(&Float:n);");
        let findings = compare_declarations(&declared, &registered);

        assert!(findings.contains(&Divergence::Arity {
            native: "Foo".into(),
            include: 2,
            plugin: 1
        }));
        assert!(findings.contains(&Divergence::ReturnTag {
            native: "Bar".into(),
            include: "bool".into(),
            plugin: String::new()
        }));
        assert!(findings.iter().any(|f| matches!(
            f,
            Divergence::ArgumentShape { native, position: 1, .. } if native == "Baz"
        )));
    }

    #[test]
    fn a_raw_native_is_compared_by_name_only() {
        // `raw` natives parse their own arguments, so the include is the only
        // place their shape is written down: presence is checked, shape is not.
        let mut registered = parse("native bool:email_send_to(...);");
        registered[0].shape_known = false;
        let declared = parse(
            "native bool:email_send_to(const to[], const subject[], const body[], {Float,_}:...);",
        );
        assert!(compare_declarations(&declared, &registered).is_empty());

        // Its absence from the include is still reported.
        assert_eq!(
            compare_declarations(&parse(""), &registered),
            vec![Divergence::MissingFromInclude {
                native: "email_send_to".into()
            }]
        );
    }

    #[test]
    fn an_alias_is_matched_by_what_implements_it() {
        // An include may present the whole surface under other names, each
        // aliasing the registered native; that is not drift.
        let declared = parse("native bool:Email_Close(account = 0) = email_close;");
        assert_eq!(declared[0].implemented_by.as_deref(), Some("email_close"));
        assert!(
            compare_declarations(&declared, &parse("native bool:email_close(account);")).is_empty()
        );

        // The shape is still compared, and reported under the declared name.
        let findings = compare_declarations(
            &declared,
            &parse("native bool:email_close(account, force);"),
        );
        assert_eq!(
            findings,
            vec![Divergence::Arity {
                native: "Email_Close".into(),
                include: 1,
                plugin: 2
            }]
        );
    }

    #[test]
    fn an_include_that_matches_reports_nothing() {
        let source = "native bool:Counter_Get(&out);\nnative Counter_Reset();";
        assert!(compare_declarations(&parse(source), &parse(source)).is_empty());
    }

    #[test]
    fn findings_come_back_in_a_stable_order() {
        let registered = parse("native Zeta();\nnative Alpha();");
        let findings = compare_declarations(&parse(""), &registered);
        let names: Vec<_> = findings
            .iter()
            .map(|f| match f {
                Divergence::MissingFromInclude { native } => native.clone(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(names, vec!["Alpha", "Zeta"]);
    }
}
