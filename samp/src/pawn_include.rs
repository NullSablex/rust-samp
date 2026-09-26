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
    /// The include is a template that does not render — checked here because a
    /// template that cannot be rendered cannot be compared either.
    Template { problem: String },
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
            Self::Template { problem } => write!(f, "{problem}"),
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
    compare_with(include_source, &crate::plugin::native_decls())
}

/// Renders an include from a name and declarations, no server involved.
///
/// The counterpart of [`compare_with`] for generating rather than checking: a
/// plugin can write its `.inc` from a test, instead of starting a server with
/// `SAMP_PAWN_INCLUDE` set.
#[must_use]
pub fn render(plugin_name: &str, decls: &[&str]) -> String {
    crate::plugin::render_include(plugin_name, decls)
}

/// Reads the declarations `#[native]` derived, as produced by the
/// `pawn_native_decls()` the plugin macro generates.
///
/// A `raw` native comes rendered commented out, its arity not being in the Rust
/// signature. The name is still registered, so it is read back here and marked
/// [`Declaration::shape_known`] `false` — its presence is compared, its shape is
/// not.
#[must_use]
pub fn registered(decls: &[&str]) -> Vec<Declaration> {
    decls
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

/// Compares an include with declarations in hand, no server involved.
///
/// This is the form a CI job wants: pass the `pawn_native_decls()` the plugin
/// macro generates and the check runs in `cargo test`.
#[must_use]
pub fn compare_with(include_source: &str, decls: &[&str]) -> Vec<Divergence> {
    // A template is checked as what it renders to: its declarations come from
    // the same place, so the comparison is about the hand-written ones around
    // them — and a template that will not render is worth hearing about here too.
    let rendered = if include_source.contains("{{") {
        match Template::new(include_source, decls).render() {
            Ok(rendered) => rendered,
            Err(errors) => {
                return errors
                    .into_iter()
                    .map(|e| Divergence::Template {
                        problem: e.to_string(),
                    })
                    .collect();
            }
        }
    } else {
        include_source.to_string()
    };

    compare_declarations(&parse(&rendered), &registered(decls))
}

/// Reads `path` and compares it with declarations in hand.
///
/// # Errors
/// Propagates the [`std::io::Error`] when the file cannot be read.
pub fn compare_file_with(
    path: impl AsRef<Path>,
    decls: &[&str],
) -> std::io::Result<Vec<Divergence>> {
    Ok(compare_with(&std::fs::read_to_string(path)?, decls))
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

/// Reads a path from `var`, honouring a per-plugin selector.
///
/// The environment belongs to the whole process, and a server can have several
/// Rust plugins loaded, each with its own include — pointed at one path, they
/// would overwrite each other's file. So a value may name the plugin it is for:
///
/// ```text
/// SAMP_PAWN_INCLUDE=counter.inc                     # whichever plugin reads it
/// SAMP_PAWN_INCLUDE=counter=counter.inc,email_samp=email.inc
/// ```
///
/// A bare path applies to every plugin, which is what a single-plugin setup
/// wants. With selectors, a plugin not named takes nothing.
pub(crate) fn path_for_plugin(var: &str) -> Option<std::ffi::OsString> {
    let value = std::env::var_os(var)?;
    let text = value.to_string_lossy();

    // A Windows path (`C:\...`) has no `=`; a selector always does.
    if !text.contains('=') {
        return Some(value);
    }

    let plugin = crate::runtime::Runtime::try_get().map_or("plugin", |rt| rt.plugin_name());
    for entry in text.split(',') {
        if let Some((name, path)) = entry.split_once('=')
            && name.trim() == plugin
        {
            return Some(std::ffi::OsString::from(path.trim()));
        }
    }
    None
}

/// Renders `template` into `out`, for the `SAMP_PAWN_INCLUDE_TEMPLATE` path.
///
/// Values for placeholders beyond the built-in ones come from the environment:
/// `SAMP_PAWN_VAR_RELEASED=2026-09-26` supplies `{{RELEASED}}`. That keeps the
/// generation usable with no code at all — start the server once with the two
/// variables set — while a plugin that wants more control uses [`Template`]
/// from a test.
///
/// Every failure is logged and otherwise ignored: producing a development
/// artifact must never take the server down.
pub(crate) fn write_from_template(template: &std::ffi::OsStr, out: &std::ffi::OsStr) {
    let shown = template.to_string_lossy().into_owned();

    let source = match std::fs::read_to_string(template) {
        Ok(source) => source,
        Err(e) => {
            crate::macros::sdk_warn!("could not read {shown}: {e}");
            return;
        }
    };

    let decls = crate::plugin::native_decls();
    let mut rendering = Template::new(&source, &decls);
    for (key, value) in std::env::vars() {
        if let Some(name) = key.strip_prefix("SAMP_PAWN_VAR_") {
            rendering = rendering.var(name, value);
        }
    }

    match rendering.write(out) {
        Ok(()) => crate::macros::sdk_info!(
            "Pawn include written to {} from {shown}",
            out.to_string_lossy()
        ),
        Err(e) => crate::macros::sdk_warn!("{shown}: {e}"),
    }
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
    let Some(path) = path_for_plugin("SAMP_PAWN_INCLUDE_CHECK") else {
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

    // -----------------------------------------------------------------------
    // Templates
    // -----------------------------------------------------------------------

    const DECLS: &[&str] = &[
        "native Counter_Increment();",
        "native bool:Counter_Get(&out);",
        "// native Counter_Send(...); // raw native — fill in the arguments",
    ];

    #[test]
    fn the_prose_stays_and_the_declarations_are_filled_in() {
        let template = "\
// My plugin v{{VERSION}}
#if defined {{GUARD}}
    #endinput
#endif
#define {{GUARD}}

// Adds one.
{{NATIVE:Counter_Increment}}

{{NATIVES}}
";
        let out = Template::new(template, DECLS)
            .plugin_name("counter")
            .var("VERSION", "1.2.3")
            .render()
            .expect("the template places every native");

        assert!(out.contains("// My plugin v1.2.3"));
        assert!(out.contains("#define _counter_included"));
        // Placed by name, under its own comment, and not repeated by {{NATIVES}}.
        assert_eq!(out.matches("native Counter_Increment();").count(), 1);
        assert!(out.contains("native bool:Counter_Get(&out);"));
    }

    #[test]
    fn a_native_can_be_placed_under_another_name() {
        let out = Template::new("{{NATIVE:Counter_Get as Counter_Read}}\n{{NATIVES}}", DECLS)
            .render()
            .unwrap();

        assert!(out.contains("native bool:Counter_Read(&out) = Counter_Get;"));
        assert!(
            !out.contains("native bool:Counter_Get(&out);"),
            "the alias replaces the original, it does not add to it"
        );
    }

    #[test]
    fn a_raw_native_can_be_placed_by_name() {
        // Its rendered form is commented out, so a template that only wants the
        // hand-written line places it and writes the arguments itself.
        let out = Template::new("{{NATIVES}}", DECLS).render().unwrap();
        assert!(out.contains("// native Counter_Send(...);"));
    }

    #[test]
    fn a_native_the_template_forgets_is_an_error() {
        let errors = Template::new("{{NATIVE:Counter_Increment}}", DECLS)
            .render()
            .expect_err("two natives are left out");

        assert!(errors.contains(&TemplateError::NativeNotPlaced {
            native: "Counter_Get".into()
        }));
        assert!(errors[0].to_string().contains("{{NATIVES}}"));
    }

    #[test]
    fn a_placeholder_with_nothing_behind_it_is_an_error() {
        let errors = Template::new("{{RELEASED}}{{NATIVES}}", DECLS)
            .render()
            .expect_err("RELEASED was never supplied");

        assert_eq!(
            errors,
            vec![TemplateError::UnknownPlaceholder {
                name: "RELEASED".into()
            }]
        );
    }

    #[test]
    fn naming_a_native_that_does_not_exist_is_an_error() {
        let errors = Template::new("{{NATIVE:Counter_Gone}}{{NATIVES}}", DECLS)
            .render()
            .expect_err("no such native");

        assert!(errors.contains(&TemplateError::UnknownNative {
            native: "Counter_Gone".into()
        }));
    }

    #[test]
    fn an_unclosed_placeholder_is_an_error() {
        let errors = Template::new("{{NATIVES}} and then {{OOPS", DECLS)
            .render()
            .expect_err("the second placeholder never closes");

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, TemplateError::UnclosedPlaceholder { .. }))
        );
    }

    #[test]
    fn version_comes_from_the_plugin_unless_the_caller_says_otherwise() {
        // The version is in every include header, so the SDK fills it in from
        // the plugin crate; passing one takes precedence.
        let out = Template::new("v{{VERSION}}\n{{NATIVES}}", DECLS)
            .var("VERSION", "9.9.9")
            .render()
            .unwrap();

        assert!(out.starts_with("v9.9.9"));
    }

    #[test]
    fn the_last_value_for_a_name_wins() {
        let out = Template::new("{{V}}{{NATIVES}}", DECLS)
            .var("V", "first")
            .var("V", "second")
            .render()
            .unwrap();

        assert!(out.starts_with("second"));
    }

    #[test]
    fn checking_a_template_compares_what_it_renders_to() {
        // The template's own declarations cannot drift; a line written by hand
        // beside them still can.
        let shaped: &[&str] = &[
            "native Counter_Increment();",
            "native bool:Counter_Get(&out);",
        ];
        assert!(compare_with("{{NATIVES}}", shaped).is_empty());

        let with_a_stale_line = "{{NATIVES}}\nnative Counter_Removed(a);";
        assert_eq!(
            compare_with(with_a_stale_line, shaped),
            vec![Divergence::NotRegistered {
                native: "Counter_Removed".into()
            }]
        );
    }

    #[test]
    fn a_raw_native_left_commented_out_is_still_reported_as_missing() {
        // `{{NATIVES}}` emits it as `#[native]` rendered it — commented out —
        // so the include does not declare it. Give the native an `args = "…"`
        // in its attribute, or write the line in the template.
        assert_eq!(
            compare_with("{{NATIVES}}", DECLS),
            vec![Divergence::MissingFromInclude {
                native: "Counter_Send".into()
            }]
        );
    }

    #[test]
    fn a_template_that_will_not_render_is_reported_by_the_check() {
        let findings = compare_with("{{NOPE}}{{NATIVES}}", DECLS);
        assert!(matches!(findings.as_slice(), [Divergence::Template { .. }]));
        assert!(findings[0].to_string().contains("NOPE"));
    }

    #[test]
    fn an_env_path_can_name_the_plugin_it_is_for() {
        // Several Rust plugins on one server read the same variable, so a value
        // may say which plugin each path is for. A bare path is for everyone.
        const VAR: &str = "SAMP_PAWN_INCLUDE_TEST_PATH";
        let _fixture = crate::test_support::exclusive();

        // SAFETY: the fixture's lock keeps this the only test touching the
        // environment, and the variable is this test's own.
        unsafe { std::env::set_var(VAR, "plain.inc") };
        assert_eq!(path_for_plugin(VAR).unwrap(), "plain.inc");

        unsafe { std::env::set_var(VAR, "counter=counter.inc, other=other.inc") };
        let plugin = crate::runtime::Runtime::try_get().map_or("plugin", |rt| rt.plugin_name());
        assert_eq!(
            path_for_plugin(VAR),
            None,
            "the test plugin ({plugin}) is not named, so it takes nothing"
        );

        unsafe { std::env::set_var(VAR, format!("{plugin}=mine.inc,other=other.inc")) };
        assert_eq!(path_for_plugin(VAR).unwrap(), "mine.inc");

        unsafe { std::env::remove_var(VAR) };
        assert_eq!(path_for_plugin(VAR), None);
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

// ---------------------------------------------------------------------------
// Generating from a template
// ---------------------------------------------------------------------------

/// Why a template could not be rendered.
///
/// Each variant is a mistake that would otherwise ship as a broken include, so
/// rendering reports it instead of guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// `{{SOMETHING}}` with no value behind it.
    UnknownPlaceholder { name: String },
    /// `{{NATIVE:Name}}` naming a native the plugin does not register.
    UnknownNative { native: String },
    /// A registered native that no placeholder emits, with no `{{NATIVES}}` to
    /// collect it: it would be missing from the include.
    NativeNotPlaced { native: String },
    /// A `{{` with no `}}` after it.
    UnclosedPlaceholder { at: usize },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPlaceholder { name } => {
                write!(
                    f,
                    "{{{{{name}}}}} has no value; pass it with .var(\"{name}\", …)"
                )
            }
            Self::UnknownNative { native } => {
                write!(
                    f,
                    "{{{{NATIVE:{native}}}}} names a native this plugin does not register"
                )
            }
            Self::NativeNotPlaced { native } => write!(
                f,
                "{native} is registered but the template never places it; \
                 name it with {{{{NATIVE:{native}}}}} or add {{{{NATIVES}}}}"
            ),
            Self::UnclosedPlaceholder { at } => {
                write!(
                    f,
                    "a placeholder opened at byte {at} is never closed with }}}}"
                )
            }
        }
    }
}

/// A Pawn include written by hand, with the declarations filled in.
///
/// The prose, the sections, the constants and the callback documentation stay in
/// the template, where they belong; every `native` line comes from the Rust
/// signature, so it cannot drift. This is the middle ground between generating
/// the whole file — which throws the documentation away — and maintaining it by
/// hand, which drifts.
///
/// Placeholders, all written `{{NAME}}`:
///
/// | Placeholder | Becomes |
/// | ----------- | ------- |
/// | `{{NATIVES}}` | every declaration not placed individually, in registration order |
/// | `{{NATIVE:Name}}` | that one declaration |
/// | `{{NATIVE:Name as Alias}}` | `native Alias(…) = Name;`, the aliased form |
/// | `{{PLUGIN}}` | the plugin's crate name |
/// | `{{GUARD}}` | `_<plugin>_included`, the usual include guard symbol |
/// | anything else | what [`Template::var`] supplied, or an error |
///
/// ```rust,no_run
/// # fn pawn_native_decls() -> Vec<&'static str> { vec![] }
/// # fn example() -> std::io::Result<()> {
/// let template = std::fs::read_to_string("include/my_plugin.inc.in")?;
///
/// samp::pawn_include::Template::new(&template, &pawn_native_decls())
///     .var("VERSION", env!("CARGO_PKG_VERSION"))
///     .write("include/my_plugin.inc")
///     .expect("the template and the natives agree");
/// # Ok(())
/// # }
/// ```
///
/// A native the template never places is an error, not a silent omission: that
/// is the drift this exists to prevent.
pub struct Template<'a> {
    source: &'a str,
    /// Each declaration as `#[native]` rendered it, paired with the native's
    /// name. Kept side by side so a placeholder naming one and `{{NATIVES}}`
    /// collecting the rest agree on which is which.
    natives: Vec<(String, String)>,
    vars: Vec<(String, String)>,
    plugin_name: String,
}

impl<'a> Template<'a> {
    /// Takes the template's text and the declarations to place into it, as
    /// `pawn_native_decls()` produces them.
    #[must_use]
    pub fn new(source: &'a str, decls: &[&str]) -> Self {
        let natives = decls
            .iter()
            .map(|decl| {
                // A `raw` native comes rendered commented out; its name is still
                // in there, and placing it by name is how a template gives it
                // the argument list the signature does not have.
                let body = decl.trim_start().trim_start_matches("//").trim_start();
                let name = parse(body)
                    .first()
                    .map(|d| d.name.clone())
                    .unwrap_or_default();
                ((*decl).to_string(), name)
            })
            .collect();

        Self {
            source,
            natives,
            vars: Vec::new(),
            plugin_name: String::new(),
        }
    }

    /// Supplies one `{{NAME}}`. The last value for a name wins.
    #[must_use]
    pub fn var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.push((name.into(), value.into()));
        self
    }

    /// Names the plugin, for `{{PLUGIN}}` and `{{GUARD}}`.
    ///
    /// Defaults to the name the plugin registered, which is its crate name.
    #[must_use]
    pub fn plugin_name(mut self, name: impl Into<String>) -> Self {
        self.plugin_name = name.into();
        self
    }

    /// Renders the include.
    ///
    /// # Errors
    /// Returns every [`TemplateError`] found, in the order they were met, so one
    /// pass names all of them rather than one per run.
    pub fn render(&self) -> Result<String, Vec<TemplateError>> {
        let mut out = String::with_capacity(self.source.len() + 256);
        let mut errors = Vec::new();
        let mut placed: Vec<usize> = Vec::new();
        let mut rest = self.source;
        let mut consumed = 0usize;

        while let Some(open) = rest.find("{{") {
            out.push_str(&rest[..open]);
            let after = &rest[open + 2..];

            let Some(close) = after.find("}}") else {
                errors.push(TemplateError::UnclosedPlaceholder {
                    at: consumed + open,
                });
                break;
            };

            let name = after[..close].trim();
            match self.expand(name, &mut placed) {
                Ok(text) => out.push_str(&text),
                Err(e) => errors.push(e),
            }

            consumed += open + 2 + close + 2;
            rest = &after[close + 2..];
        }

        if errors.is_empty() {
            out.push_str(rest);
        }

        // Everything not placed by name goes where `{{NATIVES}}` asked for it,
        // and if nothing did, its absence is the error worth reporting.
        for (i, (_, name)) in self.natives.iter().enumerate() {
            if !placed.contains(&i) {
                errors.push(TemplateError::NativeNotPlaced {
                    native: name.clone(),
                });
            }
        }

        if errors.is_empty() {
            Ok(out)
        } else {
            Err(errors)
        }
    }

    /// Renders and writes the include to `path`.
    ///
    /// # Errors
    /// The template's own errors, or the [`std::io::Error`] from writing.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), WriteError> {
        let rendered = self.render().map_err(WriteError::Template)?;
        std::fs::write(path, rendered).map_err(WriteError::Io)
    }

    /// Expands one placeholder, recording which declarations it consumed.
    fn expand(&self, name: &str, placed: &mut Vec<usize>) -> Result<String, TemplateError> {
        if name == "NATIVES" {
            let mut lines = Vec::new();
            for (i, (decl, _)) in self.natives.iter().enumerate() {
                if !placed.contains(&i) {
                    placed.push(i);
                    lines.push(decl.clone());
                }
            }
            return Ok(lines.join("\n"));
        }

        if let Some(spec) = name.strip_prefix("NATIVE:") {
            let (native, alias) = match spec.split_once(" as ") {
                Some((native, alias)) => (native.trim(), Some(alias.trim())),
                None => (spec.trim(), None),
            };
            let Some(i) = self.natives.iter().position(|(_, name)| name == native) else {
                return Err(TemplateError::UnknownNative {
                    native: native.to_string(),
                });
            };
            if !placed.contains(&i) {
                placed.push(i);
            }
            return Ok(match alias {
                Some(alias) => alias_line(&self.natives[i].0, native, alias),
                None => self.natives[i].0.clone(),
            });
        }

        if name == "PLUGIN" {
            return Ok(self.name());
        }
        // `{{VERSION}}` is the plugin crate's version unless the caller
        // supplied one: it is in every include header, and the SDK knows it.
        if name == "VERSION"
            && !self.vars.iter().any(|(key, _)| key == "VERSION")
            && let Some(rt) = crate::runtime::Runtime::try_get()
        {
            return Ok(rt.plugin_version().to_string());
        }
        if name == "GUARD" {
            return Ok(guard_symbol(&self.name()));
        }

        self.vars
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| TemplateError::UnknownPlaceholder {
                name: name.to_string(),
            })
    }

    fn name(&self) -> String {
        if self.plugin_name.is_empty() {
            crate::runtime::Runtime::try_get()
                .map_or_else(|| String::from("plugin"), |rt| rt.plugin_name().to_string())
        } else {
            self.plugin_name.clone()
        }
    }
}

/// A template that could not be rendered, or could not be written.
#[derive(Debug)]
pub enum WriteError {
    Template(Vec<TemplateError>),
    Io(std::io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Template(errors) => {
                writeln!(f, "the template could not be rendered:")?;
                for e in errors {
                    writeln!(f, "  {e}")?;
                }
                Ok(())
            }
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WriteError {}

/// Turns a declaration into its aliased form: `native Alias(…) = original;`.
fn alias_line(decl: &str, native: &str, alias: &str) -> String {
    let renamed = decl.replacen(native, alias, 1);
    match renamed.rfind(';') {
        Some(at) => format!("{} = {native};", &renamed[..at]),
        None => renamed,
    }
}

/// The include guard a plugin name produces, with what Pawn rejects replaced.
fn guard_symbol(plugin_name: &str) -> String {
    let body: String = plugin_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("_{body}_included")
}
