//! Primitive types for the Open Multiplayer ABI: `UID`, `SemanticVersion`, `StringView`,
//! `Colour`, `Vector{2,3,4}`, `ComponentType`.
//!
//! All use `#[repr(C)]` to guarantee binary layout identical to the C++ SDK's
//! `types.hpp` header — do not reorder fields.

/// 64-bit unique identifier of an Open Multiplayer component.
///
/// In the SDK, it is resolved in priority order by `samp-codegen`:
///   1. `uid:` declared explicitly in `initialize_plugin!`
///   2. `[package.metadata.samp] uid` in `Cargo.toml`
///   3. FNV-1a 64-bit of `CARGO_PKG_NAME@CARGO_PKG_VERSION` (generated and
///      written to `Cargo.toml` if none of the above options exist)
pub type UID = u64;

/// Semantic version major.minor.patch with pre-release support.
///
/// 6 bytes in memory (`#[repr(C)]` + `u16` alignment); returned by
/// `componentVersion()` via hidden pointer on both ABIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SemanticVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub prerel: u16,
}

impl SemanticVersion {
    #[must_use]
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            prerel: 0,
        }
    }

    #[must_use]
    pub const fn with_prerel(major: u8, minor: u8, patch: u8, prerel: u16) -> Self {
        Self {
            major,
            minor,
            patch,
            prerel,
        }
    }
}

/// Non-owning string — `(pointer, length)` pair, no `\0` terminator.
///
/// Layout identical to `nonstd::string_view` in the C++ SDK. 8 bytes on x86 32-bit
/// (ptr 4 + len 4). Returned by `componentName()` via hidden pointer.
///
/// `StringView` does not take ownership — the producer guarantees the pointer's
/// validity for the duration of use.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct StringView {
    pub data: *const u8,
    pub len: usize,
}

impl StringView {
    /// Creates a `StringView` from a static `&str`.
    #[must_use]
    pub fn from_static(s: &'static str) -> Self {
        Self {
            data: s.as_ptr(),
            len: s.len(),
        }
    }

    /// Converts to `&str`. Safe only if the pointer is valid and UTF-8.
    ///
    /// # Safety
    /// The pointer must be valid and point to `len` bytes of valid UTF-8
    /// for the lifetime `'a`.
    #[must_use]
    pub unsafe fn as_str<'a>(self) -> &'a str {
        let slice = unsafe { std::slice::from_raw_parts(self.data, self.len) };
        // Open Multiplayer guarantees UTF-8 strings on the SDK interfaces
        unsafe { std::str::from_utf8_unchecked(slice) }
    }

    /// Converts to `&str` with explicit UTF-8 validation.
    ///
    /// Preferable to [`as_str`] when the string content is untrusted or in
    /// contexts where defensive validation is needed.
    ///
    /// # Safety
    /// The pointer must be valid and point to `len` bytes for the lifetime `'a`.
    ///
    /// # Errors
    /// [`std::str::Utf8Error`] if the pointed-to bytes do not form valid UTF-8.
    ///
    /// [`as_str`]: Self::as_str
    pub unsafe fn try_as_str<'a>(self) -> Result<&'a str, std::str::Utf8Error> {
        let slice = unsafe { std::slice::from_raw_parts(self.data, self.len) };
        std::str::from_utf8(slice)
    }
}

/// RGBA color.
///
/// Equivalent to `Colour` in `types.hpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Colour {
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }

    #[must_use]
    pub fn from_rgba_u32(v: u32) -> Self {
        Self {
            r: ((v & 0xFF00_0000) >> 24) as u8,
            g: ((v & 0x00FF_0000) >> 16) as u8,
            b: ((v & 0x0000_FF00) >> 8) as u8,
            a: (v & 0x0000_00FF) as u8,
        }
    }

    #[must_use]
    pub fn to_rgba_u32(self) -> u32 {
        (u32::from(self.r) << 24)
            | (u32::from(self.g) << 16)
            | (u32::from(self.b) << 8)
            | u32::from(self.a)
    }

    pub const WHITE: Self = Self::rgba(0xFF, 0xFF, 0xFF, 0xFF);
    pub const BLACK: Self = Self::rgba(0x00, 0x00, 0x00, 0xFF);
    pub const NONE: Self = Self::rgba(0x00, 0x00, 0x00, 0x00);
}

/// 2D vector.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

/// 3D vector — used for world positions in SA-MP/Open Multiplayer.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 4D vector.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// Component type.
///
/// Equivalent to `ComponentType` in `component.hpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum ComponentType {
    Other = 0,
    Network = 1,
    Pool = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- SemanticVersion ---

    #[test]
    fn semantic_version_new_fields() {
        let v = SemanticVersion::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerel, 0);
    }

    #[test]
    fn semantic_version_with_prerel() {
        let v = SemanticVersion::with_prerel(1, 0, 0, 5);
        assert_eq!(v.prerel, 5);
    }

    #[test]
    fn semantic_version_equality() {
        assert_eq!(SemanticVersion::new(1, 2, 3), SemanticVersion::new(1, 2, 3));
        assert_ne!(SemanticVersion::new(1, 2, 3), SemanticVersion::new(1, 2, 4));
    }

    #[test]
    fn semantic_version_clone() {
        let v = SemanticVersion::new(2, 0, 0);
        assert_eq!(v, v);
    }

    // --- StringView ---

    #[test]
    fn stringview_from_static_len() {
        let sv = StringView::from_static("hello");
        assert_eq!(sv.len, 5);
        assert!(!sv.data.is_null());
    }

    #[test]
    fn stringview_from_static_empty() {
        let sv = StringView::from_static("");
        assert_eq!(sv.len, 0);
    }

    #[test]
    fn stringview_as_str_roundtrip() {
        let sv = StringView::from_static("rust-samp");
        let s = unsafe { sv.as_str() };
        assert_eq!(s, "rust-samp");
    }

    #[test]
    fn stringview_try_as_str_valid_utf8() {
        let sv = StringView::from_static("naïve");
        let result = unsafe { sv.try_as_str() };
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "naïve");
    }

    #[test]
    fn stringview_try_as_str_invalid_utf8_returns_err() {
        let bad = [0xFF_u8, 0xFE];
        let sv = StringView {
            data: bad.as_ptr(),
            len: bad.len(),
        };
        let result = unsafe { sv.try_as_str() };
        assert!(result.is_err());
    }

    // --- Colour ---

    #[test]
    fn colour_rgba_fields() {
        let c = Colour::rgba(1, 2, 3, 4);
        assert_eq!((c.r, c.g, c.b, c.a), (1, 2, 3, 4));
    }

    #[test]
    fn colour_rgb_has_full_alpha() {
        let c = Colour::rgb(10, 20, 30);
        assert_eq!(c.a, 0xFF);
    }

    #[test]
    fn colour_from_to_rgba_u32_roundtrip() {
        let original = 0xDEAD_BEEF_u32;
        let c = Colour::from_rgba_u32(original);
        assert_eq!(c.to_rgba_u32(), original);
    }

    #[test]
    fn colour_white_constant() {
        assert_eq!(Colour::WHITE, Colour::rgba(0xFF, 0xFF, 0xFF, 0xFF));
    }

    #[test]
    fn colour_black_constant() {
        assert_eq!(Colour::BLACK, Colour::rgba(0x00, 0x00, 0x00, 0xFF));
    }

    #[test]
    fn colour_none_is_transparent() {
        assert_eq!(Colour::NONE.a, 0x00);
    }

    // --- Vector2 / Vector3 / Vector4 ---

    #[test]
    fn vector2_fields() {
        let v = Vector2 { x: 1.0, y: 2.0 };
        assert_eq!((v.x, v.y), (1.0, 2.0));
    }

    #[test]
    fn vector3_fields() {
        let v = Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        assert_eq!(v.z.to_bits(), 3.0_f32.to_bits());
    }

    #[test]
    fn vector4_fields() {
        let v = Vector4 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        assert_eq!(v.w.to_bits(), 4.0_f32.to_bits());
    }

    #[test]
    fn vectors_clone_and_eq() {
        let v = Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        assert_eq!(v, v);
    }

    // --- ComponentType ---

    #[test]
    fn component_type_discriminants() {
        assert_eq!(ComponentType::Other as i32, 0);
        assert_eq!(ComponentType::Network as i32, 1);
        assert_eq!(ComponentType::Pool as i32, 2);
    }
}
