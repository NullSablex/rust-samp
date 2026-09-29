//! What a C++ signature, as the header spells it, says about the call.

use std::collections::HashMap;

/// Top-level parameters of `f(a, b<c, d>, e)`, as written.
pub fn split_params(signature: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (signature.find('('), signature.rfind(')')) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in signature[open + 1..close].chars() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            _ => {}
        }
        if c == ',' && depth == 0 {
            out.push(current.trim().to_owned());
            current.clear();
        } else {
            current.push(c);
        }
    }
    out.push(current.trim().to_owned());
    out.retain(|p| !p.is_empty());
    out
}

/// The method's name in `Ret Class::method(args)`.
pub fn method_name(signature: &str) -> &str {
    let head = &signature[..signature.find('(').unwrap_or(signature.len())];
    head.rsplit("::").next().unwrap_or(head)
}

fn unqualified(text: &str) -> String {
    text.replace("const", "").trim().to_owned()
}

/// The bytes a `thiscall` callee pops, per by-value argument type, beyond
/// what fits a word; everything narrower still takes a word.
pub struct ArgBytes {
    sizes: HashMap<String, usize>,
    /// Types returned through a pointer pushed with the arguments.
    by_pointer: Vec<String>,
}

impl ArgBytes {
    /// The fixed table, plus the mirrored structs' MSVC sizes.
    pub fn new(mirrored: &HashMap<String, usize>) -> Self {
        let mut sizes: HashMap<String, usize> = [
            ("Vector2", 8),
            ("Vector3", 12),
            ("Vector4", 16),
            ("StringView", 8),
            ("Colour", 4),
            ("int64_t", 8),
            ("uint64_t", 8),
            ("UID", 8),
            ("long long", 8),
            ("unsigned long long", 8),
            ("double", 8),
            // `std::chrono` under Microsoft's library: minutes and hours count
            // in an `int`.
            ("Milliseconds", 8),
            ("Seconds", 8),
            ("Minutes", 4),
            ("Hours", 4),
            ("Microseconds", 8),
            ("TimePoint", 8),
            ("WorldTimePoint", 8),
            ("GangZonePos", 16),
            ("GTAQuat", 16),
            ("SemanticVersion", 8),
        ]
        .into_iter()
        .map(|(name, size)| (name.to_owned(), size))
        .collect();
        sizes.extend(
            mirrored
                .iter()
                .map(|(name, size)| (name.clone(), size.div_ceil(4) * 4)),
        );
        let mut by_pointer: Vec<String> = [
            "Vector2",
            "Vector3",
            "Vector4",
            "StringView",
            "Colour",
            "Milliseconds",
            "Seconds",
            "Minutes",
            "Hours",
            "GangZonePos",
            "GTAQuat",
            "SemanticVersion",
        ]
        .map(str::to_owned)
        .to_vec();
        by_pointer.extend(mirrored.keys().cloned());
        Self { sizes, by_pointer }
    }

    /// What the MSVC callee pops for `signature`, the hidden return pointer
    /// included: a struct returned by value arrives through a pointer pushed
    /// with the arguments.
    pub fn of(&self, signature: &str) -> usize {
        let head = &signature[..signature.find('(').unwrap_or(signature.len())];
        let ret = unqualified(head.rsplit_once(' ').map_or(head, |(ret, _)| ret));
        let mut total = if self.by_pointer.contains(&ret) || ret.starts_with("Pair<") {
            4
        } else {
            0
        };
        for param in split_params(signature) {
            let bare = unqualified(&param);
            total += if bare.ends_with('&') || bare.ends_with('*') {
                4
            } else if bare.starts_with("Span<")
                || bare.starts_with("span<")
                || bare.starts_with("nonstd::span_lite::span<")
            {
                8
            } else {
                self.sizes.get(&bare).copied().unwrap_or(4)
            };
        }
        total
    }
}

/// Whether a demangled symbol takes what the header's signature does.
///
/// The spellings differ — `StringView` demangles to the vendored view type,
/// `Vector2` to a `glm` vector — so the check is by count, and by exact name
/// only for the builtin and interface types that spell the same both ways.
pub fn same_params(signature: &str, demangled: &str) -> bool {
    let (want, got) = (split_params(signature), split_params(demangled));
    want.len() == got.len()
        && want.iter().zip(&got).all(|(w, g)| {
            let squash = |s: &str| unqualified(s).replace(' ', "");
            let (w, g) = (squash(w), squash(g));
            let plain = w.trim_end_matches(['&', '*']);
            let comparable = matches!(plain, "int" | "bool" | "float") || plain.starts_with('I');
            !comparable || w == g
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_split_at_top_level_only() {
        assert_eq!(
            split_params("bool ICore::sha256(StringView, StringView, StaticArray<char, 64 + 1> &)")
                .len(),
            3
        );
        assert!(split_params("void IPlayer::kick()").is_empty());
        assert_eq!(
            method_name("void IPlayer::setTime(Hours, Minutes)"),
            "setTime"
        );
    }

    #[test]
    fn popped_bytes_follow_the_arguments() {
        let bytes = ArgBytes::new(&HashMap::from([("WeaponSlotData".to_owned(), 8)]));
        assert_eq!(bytes.of("void IPlayer::setTime(Hours, Minutes)"), 8);
        assert_eq!(bytes.of("Vector3 IActor::getPosition()"), 4);
        assert_eq!(bytes.of("void IPlayer::giveWeapon(WeaponSlotData)"), 8);
        assert_eq!(
            bytes.of("size_t IConfig::getStrings(StringView, Span<StringView>)"),
            16
        );
        assert_eq!(bytes.of("Pair<int, int> IVehicle::getColour()"), 4);
    }

    #[test]
    fn overloads_compare_their_plain_parameters() {
        assert!(same_params(
            "void X::beginEditing(IObject &)",
            "P::beginEditing(IObject&)"
        ));
        assert!(!same_params(
            "void X::beginEditing(IObject &)",
            "P::beginEditing(IPlayerObject&)"
        ));
        assert!(!same_params(
            "X::create(Vector2, int)",
            "P::create(glm::vec<2, float>, nonstd::string_view)"
        ));
    }
}
