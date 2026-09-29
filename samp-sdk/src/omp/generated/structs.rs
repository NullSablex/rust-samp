//! Structs the open.mp headers pass by value or by reference — generated
//! by `cargo xtask gen-omp`. Do not edit.
//!
//! Each mirrors its C++ struct field for field. The layout is clang's for
//! both ABIs, and the assertions below each struct fail the build if the
//! Rust one differs from it. An anonymous union becomes a named Rust
//! union field (`u0`, ...); a run of bit-fields becomes one integer
//! (`bits_*`), first declared in the lowest bits.

#![allow(unused_imports, clippy::struct_field_names)]

use crate::omp::containers::{FlatSet, HybridString, Pair, Span};
use crate::omp::types::{
    Colour, GTAQuat, Hours, Microseconds, Milliseconds, Minutes, Seconds, TimePoint, Vector2,
    Vector3, Vector4, WorldTimePoint,
};
use crate::omp::vtable::VirtualReturn;
use crate::omp::*;

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ConsoleCommandSenderData1 {
    /// `player`.
    pub player: *mut IPlayer,
    /// `handler`.
    pub handler: *mut std::ffi::c_void,
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ObjectMaterialData0 {
    /// `model`.
    pub model: i32,
    /// `(anonymous)`.
    pub s1: ObjectMaterialData0_1,
}

/// An anonymous struct inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ObjectMaterialData0_1 {
    /// `materialSize`.
    pub material_size: u8,
    /// `fontSize`.
    pub font_size: u8,
    /// `alignment`.
    pub alignment: u8,
    /// `bold`.
    pub bold: bool,
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ObjectMaterialData1 {
    /// `materialColour`.
    pub material_colour: Colour,
    /// `fontColour`.
    pub font_colour: Colour,
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union PeerAddress1 {
    /// `v4`.
    pub v4: u32,
    /// `v6`.
    pub v6: PeerAddress1_v6,
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union PeerAddress1_v6 {
    /// `segments`.
    pub segments: [u16; 8],
    /// `bytes`.
    pub bytes: [u8; 16],
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union VehicleDriverSyncPacket14 {
    /// `AdditionalKeyWeapon`.
    pub additional_key_weapon: u8,
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union VehicleDriverSyncPacket15 {
    /// `HydraThrustAngle`.
    pub hydra_thrust_angle: u32,
    /// `TrainSpeed`.
    pub train_speed: f32,
}

/// An anonymous union inside a mirrored struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub union VehiclePassengerSyncPacket2 {
    /// `DriveBySeatAdditionalKeyWeapon`.
    pub drive_by_seat_additional_key_weapon: u16,
}

/// `ActorSpawnData` in `Server/Components/Actors/actors.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorSpawnData {
    /// `position`.
    pub position: Vector3,
    /// `facingAngle`.
    pub facing_angle: f32,
    /// `skin`.
    pub skin: i32,
}

impl Default for ActorSpawnData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            position: Vector3::ZERO,
            facing_angle: 0.0,
            skin: 0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<ActorSpawnData>() == 20);
    assert!(std::mem::offset_of!(ActorSpawnData, position) == 0);
    assert!(std::mem::offset_of!(ActorSpawnData, facing_angle) == 12);
    assert!(std::mem::offset_of!(ActorSpawnData, skin) == 16);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<ActorSpawnData>() == 20);
    assert!(std::mem::offset_of!(ActorSpawnData, position) == 0);
    assert!(std::mem::offset_of!(ActorSpawnData, facing_angle) == 12);
    assert!(std::mem::offset_of!(ActorSpawnData, skin) == 16);
};

impl VirtualReturn for ActorSpawnData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `AnimationData` in `anim.hpp`.
///
/// The C++ side copies it with a constructor, so it travels by
/// reference only: build one here and pass `&`, or read the server's.
#[repr(C)]
pub struct AnimationData {
    /// `delta`.
    pub delta: f32,
    /// `loop`.
    pub r#loop: bool,
    /// `lockX`.
    pub lock_x: bool,
    /// `lockY`.
    pub lock_y: bool,
    /// `freeze`.
    pub freeze: bool,
    /// `time`.
    pub time: u32,
    /// `lib`.
    pub lib: HybridString<16>,
    /// `name`.
    pub name: HybridString<24>,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<AnimationData>() == 60);
    assert!(std::mem::offset_of!(AnimationData, delta) == 0);
    assert!(std::mem::offset_of!(AnimationData, r#loop) == 4);
    assert!(std::mem::offset_of!(AnimationData, lock_x) == 5);
    assert!(std::mem::offset_of!(AnimationData, lock_y) == 6);
    assert!(std::mem::offset_of!(AnimationData, freeze) == 7);
    assert!(std::mem::offset_of!(AnimationData, time) == 8);
    assert!(std::mem::offset_of!(AnimationData, lib) == 12);
    assert!(std::mem::offset_of!(AnimationData, name) == 32);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<AnimationData>() == 60);
    assert!(std::mem::offset_of!(AnimationData, delta) == 0);
    assert!(std::mem::offset_of!(AnimationData, r#loop) == 4);
    assert!(std::mem::offset_of!(AnimationData, lock_x) == 5);
    assert!(std::mem::offset_of!(AnimationData, lock_y) == 6);
    assert!(std::mem::offset_of!(AnimationData, freeze) == 7);
    assert!(std::mem::offset_of!(AnimationData, time) == 8);
    assert!(std::mem::offset_of!(AnimationData, lib) == 12);
    assert!(std::mem::offset_of!(AnimationData, name) == 32);
};

/// `BanEntry` in `network.hpp`.
///
/// The C++ side copies it with a constructor, so it travels by
/// reference only: build one here and pass `&`, or read the server's.
#[repr(C)]
pub struct BanEntry {
    /// `address`.
    pub address: HybridString<46>,
    /// `time`.
    pub time: WorldTimePoint,
    /// `name`.
    pub name: HybridString<25>,
    /// `reason`.
    pub reason: HybridString<32>,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<BanEntry>() == 128);
    assert!(std::mem::offset_of!(BanEntry, address) == 0);
    assert!(std::mem::offset_of!(BanEntry, time) == 52);
    assert!(std::mem::offset_of!(BanEntry, name) == 60);
    assert!(std::mem::offset_of!(BanEntry, reason) == 92);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<BanEntry>() == 136);
    assert!(std::mem::offset_of!(BanEntry, address) == 0);
    assert!(std::mem::offset_of!(BanEntry, time) == 56);
    assert!(std::mem::offset_of!(BanEntry, name) == 64);
    assert!(std::mem::offset_of!(BanEntry, reason) == 96);
};

/// `ConsoleCommandSenderData` in `Server/Components/Console/console.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConsoleCommandSenderData {
    /// `sender`.
    pub sender: i32,
    /// `(anonymous union)`.
    pub u1: ConsoleCommandSenderData1,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<ConsoleCommandSenderData>() == 8);
    assert!(std::mem::offset_of!(ConsoleCommandSenderData, sender) == 0);
    assert!(std::mem::offset_of!(ConsoleCommandSenderData, u1) == 4);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<ConsoleCommandSenderData>() == 8);
    assert!(std::mem::offset_of!(ConsoleCommandSenderData, sender) == 0);
    assert!(std::mem::offset_of!(ConsoleCommandSenderData, u1) == 4);
};

impl VirtualReturn for ConsoleCommandSenderData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `LegacyDBResult` in `Server/Components/Databases/databases.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyDBResult {
    /// `rows`.
    pub rows: i32,
    /// `columns`.
    pub columns: i32,
    /// `results`.
    pub results: *mut *mut i8,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<LegacyDBResult>() == 12);
    assert!(std::mem::offset_of!(LegacyDBResult, rows) == 0);
    assert!(std::mem::offset_of!(LegacyDBResult, columns) == 4);
    assert!(std::mem::offset_of!(LegacyDBResult, results) == 8);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<LegacyDBResult>() == 12);
    assert!(std::mem::offset_of!(LegacyDBResult, rows) == 0);
    assert!(std::mem::offset_of!(LegacyDBResult, columns) == 4);
    assert!(std::mem::offset_of!(LegacyDBResult, results) == 8);
};

impl VirtualReturn for LegacyDBResult {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `NetworkID` in `network.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetworkID {
    /// `address`.
    pub address: PeerAddress,
    /// `port`.
    pub port: u16,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<NetworkID>() == 24);
    assert!(std::mem::offset_of!(NetworkID, address) == 0);
    assert!(std::mem::offset_of!(NetworkID, port) == 20);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<NetworkID>() == 24);
    assert!(std::mem::offset_of!(NetworkID, address) == 0);
    assert!(std::mem::offset_of!(NetworkID, port) == 20);
};

impl VirtualReturn for NetworkID {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `ObjectAttachmentData` in `Server/Components/Objects/objects.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectAttachmentData {
    /// `type`.
    pub r#type: u8,
    /// `syncRotation`.
    pub sync_rotation: bool,
    /// `ID`.
    pub id: i32,
    /// `offset`.
    pub offset: Vector3,
    /// `rotation`.
    pub rotation: Vector3,
}

impl Default for ObjectAttachmentData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            r#type: 0,
            sync_rotation: false,
            id: 0,
            offset: Vector3::ZERO,
            rotation: Vector3::ZERO,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<ObjectAttachmentData>() == 32);
    assert!(std::mem::offset_of!(ObjectAttachmentData, r#type) == 0);
    assert!(std::mem::offset_of!(ObjectAttachmentData, sync_rotation) == 1);
    assert!(std::mem::offset_of!(ObjectAttachmentData, id) == 4);
    assert!(std::mem::offset_of!(ObjectAttachmentData, offset) == 8);
    assert!(std::mem::offset_of!(ObjectAttachmentData, rotation) == 20);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<ObjectAttachmentData>() == 32);
    assert!(std::mem::offset_of!(ObjectAttachmentData, r#type) == 0);
    assert!(std::mem::offset_of!(ObjectAttachmentData, sync_rotation) == 1);
    assert!(std::mem::offset_of!(ObjectAttachmentData, id) == 4);
    assert!(std::mem::offset_of!(ObjectAttachmentData, offset) == 8);
    assert!(std::mem::offset_of!(ObjectAttachmentData, rotation) == 20);
};

impl VirtualReturn for ObjectAttachmentData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `ObjectAttachmentSlotData` in `Server/Components/Objects/objects.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectAttachmentSlotData {
    /// `model`.
    pub model: i32,
    /// `bone`.
    pub bone: i32,
    /// `offset`.
    pub offset: Vector3,
    /// `rotation`.
    pub rotation: Vector3,
    /// `scale`.
    pub scale: Vector3,
    /// `colour1`.
    pub colour1: Colour,
    /// `colour2`.
    pub colour2: Colour,
}

impl Default for ObjectAttachmentSlotData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            model: 0,
            bone: 0,
            offset: Vector3::ZERO,
            rotation: Vector3::ZERO,
            scale: Vector3::ZERO,
            colour1: Colour::default(),
            colour2: Colour::default(),
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<ObjectAttachmentSlotData>() == 52);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, model) == 0);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, bone) == 4);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, offset) == 8);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, rotation) == 20);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, scale) == 32);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, colour1) == 44);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, colour2) == 48);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<ObjectAttachmentSlotData>() == 52);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, model) == 0);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, bone) == 4);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, offset) == 8);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, rotation) == 20);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, scale) == 32);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, colour1) == 44);
    assert!(std::mem::offset_of!(ObjectAttachmentSlotData, colour2) == 48);
};

impl VirtualReturn for ObjectAttachmentSlotData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `ObjectMaterialData` in `Server/Components/Objects/objects.hpp`.
///
/// The C++ side copies it with a constructor, so it travels by
/// reference only: build one here and pass `&`, or read the server's.
#[repr(C)]
pub struct ObjectMaterialData {
    /// `(anonymous union)`.
    pub u0: ObjectMaterialData0,
    /// `(anonymous union)`.
    pub u1: ObjectMaterialData1,
    /// `backgroundColour`.
    pub background_colour: Colour,
    /// `textOrTXD`.
    pub text_or_txd: HybridString<32>,
    /// `fontOrTexture`.
    pub font_or_texture: HybridString<32>,
    /// `type`.
    pub r#type: u8,
    /// `used`.
    pub used: bool,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<ObjectMaterialData>() == 88);
    assert!(std::mem::offset_of!(ObjectMaterialData, u0) == 0);
    assert!(std::mem::offset_of!(ObjectMaterialData, u1) == 4);
    assert!(std::mem::offset_of!(ObjectMaterialData, background_colour) == 8);
    assert!(std::mem::offset_of!(ObjectMaterialData, text_or_txd) == 12);
    assert!(std::mem::offset_of!(ObjectMaterialData, font_or_texture) == 48);
    assert!(std::mem::offset_of!(ObjectMaterialData, r#type) == 84);
    assert!(std::mem::offset_of!(ObjectMaterialData, used) == 85);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<ObjectMaterialData>() == 88);
    assert!(std::mem::offset_of!(ObjectMaterialData, u0) == 0);
    assert!(std::mem::offset_of!(ObjectMaterialData, u1) == 4);
    assert!(std::mem::offset_of!(ObjectMaterialData, background_colour) == 8);
    assert!(std::mem::offset_of!(ObjectMaterialData, text_or_txd) == 12);
    assert!(std::mem::offset_of!(ObjectMaterialData, font_or_texture) == 48);
    assert!(std::mem::offset_of!(ObjectMaterialData, r#type) == 84);
    assert!(std::mem::offset_of!(ObjectMaterialData, used) == 85);
};

/// `ObjectMoveData` in `Server/Components/Objects/objects.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectMoveData {
    /// `targetPos`.
    pub target_pos: Vector3,
    /// `targetRot`.
    pub target_rot: Vector3,
    /// `speed`.
    pub speed: f32,
}

impl Default for ObjectMoveData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            target_pos: Vector3::ZERO,
            target_rot: Vector3::ZERO,
            speed: 0.0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<ObjectMoveData>() == 28);
    assert!(std::mem::offset_of!(ObjectMoveData, target_pos) == 0);
    assert!(std::mem::offset_of!(ObjectMoveData, target_rot) == 12);
    assert!(std::mem::offset_of!(ObjectMoveData, speed) == 24);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<ObjectMoveData>() == 28);
    assert!(std::mem::offset_of!(ObjectMoveData, target_pos) == 0);
    assert!(std::mem::offset_of!(ObjectMoveData, target_rot) == 12);
    assert!(std::mem::offset_of!(ObjectMoveData, speed) == 24);
};

impl VirtualReturn for ObjectMoveData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PeerAddress` in `network.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PeerAddress {
    /// `ipv6`.
    pub ipv6: bool,
    /// `(anonymous union)`.
    pub u1: PeerAddress1,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PeerAddress>() == 20);
    assert!(std::mem::offset_of!(PeerAddress, ipv6) == 0);
    assert!(std::mem::offset_of!(PeerAddress, u1) == 4);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PeerAddress>() == 20);
    assert!(std::mem::offset_of!(PeerAddress, ipv6) == 0);
    assert!(std::mem::offset_of!(PeerAddress, u1) == 4);
};

impl VirtualReturn for PeerAddress {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PeerNetworkData` in `network.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PeerNetworkData {
    /// `network`.
    pub network: *mut std::ffi::c_void,
    /// `networkID`.
    pub network_id: NetworkID,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PeerNetworkData>() == 28);
    assert!(std::mem::offset_of!(PeerNetworkData, network) == 0);
    assert!(std::mem::offset_of!(PeerNetworkData, network_id) == 4);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PeerNetworkData>() == 28);
    assert!(std::mem::offset_of!(PeerNetworkData, network) == 0);
    assert!(std::mem::offset_of!(PeerNetworkData, network_id) == 4);
};

impl VirtualReturn for PeerNetworkData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PeerRequestParams` in `network.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PeerRequestParams {
    /// `version`.
    pub version: u8,
    /// `versionName`.
    pub version_name: StringView,
    /// `bot`.
    pub bot: bool,
    /// `name`.
    pub name: StringView,
    /// `serial`.
    pub serial: StringView,
    /// Bit-fields, from bit 0: `isUsingOfficialClient`: 1, `isUsingOmp`: 1.
    pub bits_is_using_official_client: u8,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PeerRequestParams>() == 36);
    assert!(std::mem::offset_of!(PeerRequestParams, version) == 0);
    assert!(std::mem::offset_of!(PeerRequestParams, version_name) == 4);
    assert!(std::mem::offset_of!(PeerRequestParams, bot) == 12);
    assert!(std::mem::offset_of!(PeerRequestParams, name) == 16);
    assert!(std::mem::offset_of!(PeerRequestParams, serial) == 24);
    assert!(std::mem::offset_of!(PeerRequestParams, bits_is_using_official_client) == 32);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PeerRequestParams>() == 36);
    assert!(std::mem::offset_of!(PeerRequestParams, version) == 0);
    assert!(std::mem::offset_of!(PeerRequestParams, version_name) == 4);
    assert!(std::mem::offset_of!(PeerRequestParams, bot) == 12);
    assert!(std::mem::offset_of!(PeerRequestParams, name) == 16);
    assert!(std::mem::offset_of!(PeerRequestParams, serial) == 24);
    assert!(std::mem::offset_of!(PeerRequestParams, bits_is_using_official_client) == 32);
};

impl VirtualReturn for PeerRequestParams {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PlayerAimData` in `player.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerAimData {
    /// `camFrontVector`.
    pub cam_front_vector: Vector3,
    /// `camPos`.
    pub cam_pos: Vector3,
    /// `aimZ`.
    pub aim_z: f32,
    /// `camZoom`.
    pub cam_zoom: f32,
    /// `aspectRatio`.
    pub aspect_ratio: f32,
    /// `weaponState`.
    pub weapon_state: i8,
    /// `camMode`.
    pub cam_mode: u8,
}

impl Default for PlayerAimData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            cam_front_vector: Vector3::ZERO,
            cam_pos: Vector3::ZERO,
            aim_z: 0.0,
            cam_zoom: 0.0,
            aspect_ratio: 0.0,
            weapon_state: 0,
            cam_mode: 0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PlayerAimData>() == 40);
    assert!(std::mem::offset_of!(PlayerAimData, cam_front_vector) == 0);
    assert!(std::mem::offset_of!(PlayerAimData, cam_pos) == 12);
    assert!(std::mem::offset_of!(PlayerAimData, aim_z) == 24);
    assert!(std::mem::offset_of!(PlayerAimData, cam_zoom) == 28);
    assert!(std::mem::offset_of!(PlayerAimData, aspect_ratio) == 32);
    assert!(std::mem::offset_of!(PlayerAimData, weapon_state) == 36);
    assert!(std::mem::offset_of!(PlayerAimData, cam_mode) == 37);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PlayerAimData>() == 40);
    assert!(std::mem::offset_of!(PlayerAimData, cam_front_vector) == 0);
    assert!(std::mem::offset_of!(PlayerAimData, cam_pos) == 12);
    assert!(std::mem::offset_of!(PlayerAimData, aim_z) == 24);
    assert!(std::mem::offset_of!(PlayerAimData, cam_zoom) == 28);
    assert!(std::mem::offset_of!(PlayerAimData, aspect_ratio) == 32);
    assert!(std::mem::offset_of!(PlayerAimData, weapon_state) == 36);
    assert!(std::mem::offset_of!(PlayerAimData, cam_mode) == 37);
};

impl VirtualReturn for PlayerAimData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PlayerAnimationData` in `player.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerAnimationData {
    /// `ID`.
    pub id: u16,
    /// `flags`.
    pub flags: u16,
}

impl Default for PlayerAnimationData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self { id: 0, flags: 0 }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PlayerAnimationData>() == 4);
    assert!(std::mem::offset_of!(PlayerAnimationData, id) == 0);
    assert!(std::mem::offset_of!(PlayerAnimationData, flags) == 2);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PlayerAnimationData>() == 4);
    assert!(std::mem::offset_of!(PlayerAnimationData, id) == 0);
    assert!(std::mem::offset_of!(PlayerAnimationData, flags) == 2);
};

impl VirtualReturn for PlayerAnimationData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PlayerClass` in `Server/Components/Classes/classes.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerClass {
    /// `team`.
    pub team: i32,
    /// `skin`.
    pub skin: i32,
    /// `spawn`.
    pub spawn: Vector3,
    /// `angle`.
    pub angle: f32,
    /// `weapons`.
    pub weapons: [WeaponSlotData; 13],
}

impl Default for PlayerClass {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            team: 0,
            skin: 0,
            spawn: Vector3::ZERO,
            angle: 0.0,
            weapons: [WeaponSlotData::default(); 13],
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PlayerClass>() == 128);
    assert!(std::mem::offset_of!(PlayerClass, team) == 0);
    assert!(std::mem::offset_of!(PlayerClass, skin) == 4);
    assert!(std::mem::offset_of!(PlayerClass, spawn) == 8);
    assert!(std::mem::offset_of!(PlayerClass, angle) == 20);
    assert!(std::mem::offset_of!(PlayerClass, weapons) == 24);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PlayerClass>() == 128);
    assert!(std::mem::offset_of!(PlayerClass, team) == 0);
    assert!(std::mem::offset_of!(PlayerClass, skin) == 4);
    assert!(std::mem::offset_of!(PlayerClass, spawn) == 8);
    assert!(std::mem::offset_of!(PlayerClass, angle) == 20);
    assert!(std::mem::offset_of!(PlayerClass, weapons) == 24);
};

impl VirtualReturn for PlayerClass {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PlayerKeyData` in `player.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerKeyData {
    /// `keys`.
    pub keys: u32,
    /// `upDown`.
    pub up_down: i16,
    /// `leftRight`.
    pub left_right: i16,
}

impl Default for PlayerKeyData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            keys: 0,
            up_down: 0,
            left_right: 0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PlayerKeyData>() == 8);
    assert!(std::mem::offset_of!(PlayerKeyData, keys) == 0);
    assert!(std::mem::offset_of!(PlayerKeyData, up_down) == 4);
    assert!(std::mem::offset_of!(PlayerKeyData, left_right) == 6);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PlayerKeyData>() == 8);
    assert!(std::mem::offset_of!(PlayerKeyData, keys) == 0);
    assert!(std::mem::offset_of!(PlayerKeyData, up_down) == 4);
    assert!(std::mem::offset_of!(PlayerKeyData, left_right) == 6);
};

impl VirtualReturn for PlayerKeyData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PlayerSpectateData` in `player.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerSpectateData {
    /// `spectating`.
    pub spectating: bool,
    /// `spectateID`.
    pub spectate_id: i32,
    /// `type`.
    pub r#type: i32,
}

impl Default for PlayerSpectateData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            spectating: false,
            spectate_id: 0,
            r#type: 0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PlayerSpectateData>() == 12);
    assert!(std::mem::offset_of!(PlayerSpectateData, spectating) == 0);
    assert!(std::mem::offset_of!(PlayerSpectateData, spectate_id) == 4);
    assert!(std::mem::offset_of!(PlayerSpectateData, r#type) == 8);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PlayerSpectateData>() == 12);
    assert!(std::mem::offset_of!(PlayerSpectateData, spectating) == 0);
    assert!(std::mem::offset_of!(PlayerSpectateData, spectate_id) == 4);
    assert!(std::mem::offset_of!(PlayerSpectateData, r#type) == 8);
};

impl VirtualReturn for PlayerSpectateData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `PlayerSurfingData` in `player.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerSurfingData {
    /// `type`.
    pub r#type: i32,
    /// `ID`.
    pub id: i32,
    /// `offset`.
    pub offset: Vector3,
}

impl Default for PlayerSurfingData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            r#type: 0,
            id: 0,
            offset: Vector3::ZERO,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<PlayerSurfingData>() == 20);
    assert!(std::mem::offset_of!(PlayerSurfingData, r#type) == 0);
    assert!(std::mem::offset_of!(PlayerSurfingData, id) == 4);
    assert!(std::mem::offset_of!(PlayerSurfingData, offset) == 8);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<PlayerSurfingData>() == 20);
    assert!(std::mem::offset_of!(PlayerSurfingData, r#type) == 0);
    assert!(std::mem::offset_of!(PlayerSurfingData, id) == 4);
    assert!(std::mem::offset_of!(PlayerSurfingData, offset) == 8);
};

impl VirtualReturn for PlayerSurfingData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `TextLabelAttachmentData` in `Server/Components/TextLabels/textlabels.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLabelAttachmentData {
    /// `playerID`.
    pub player_id: i32,
    /// `vehicleID`.
    pub vehicle_id: i32,
}

impl Default for TextLabelAttachmentData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            player_id: 65535,
            vehicle_id: 65535,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<TextLabelAttachmentData>() == 8);
    assert!(std::mem::offset_of!(TextLabelAttachmentData, player_id) == 0);
    assert!(std::mem::offset_of!(TextLabelAttachmentData, vehicle_id) == 4);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<TextLabelAttachmentData>() == 8);
    assert!(std::mem::offset_of!(TextLabelAttachmentData, player_id) == 0);
    assert!(std::mem::offset_of!(TextLabelAttachmentData, vehicle_id) == 4);
};

impl VirtualReturn for TextLabelAttachmentData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `VehicleDriverSyncPacket` in `Server/Components/Vehicles/vehicles.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VehicleDriverSyncPacket {
    /// `PlayerID`.
    pub player_id: i32,
    /// `VehicleID`.
    pub vehicle_id: u16,
    /// `LeftRight`.
    pub left_right: u16,
    /// `UpDown`.
    pub up_down: u16,
    /// `Keys`.
    pub keys: u16,
    /// `Rotation`.
    pub rotation: GTAQuat,
    /// `Position`.
    pub position: Vector3,
    /// `Velocity`.
    pub velocity: Vector3,
    /// `Health`.
    pub health: f32,
    /// `PlayerHealthArmour`.
    pub player_health_armour: Vector2,
    /// `Siren`.
    pub siren: u8,
    /// `LandingGear`.
    pub landing_gear: u8,
    /// `TrailerID`.
    pub trailer_id: u16,
    /// `HasTrailer`.
    pub has_trailer: bool,
    /// `(anonymous union)`.
    pub u14: VehicleDriverSyncPacket14,
    /// `(anonymous union)`.
    pub u15: VehicleDriverSyncPacket15,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<VehicleDriverSyncPacket>() == 76);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, player_id) == 0);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, vehicle_id) == 4);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, left_right) == 6);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, up_down) == 8);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, keys) == 10);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, rotation) == 12);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, position) == 28);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, velocity) == 40);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, health) == 52);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, player_health_armour) == 56);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, siren) == 64);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, landing_gear) == 65);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, trailer_id) == 66);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, has_trailer) == 68);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, u14) == 69);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, u15) == 72);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<VehicleDriverSyncPacket>() == 76);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, player_id) == 0);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, vehicle_id) == 4);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, left_right) == 6);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, up_down) == 8);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, keys) == 10);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, rotation) == 12);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, position) == 28);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, velocity) == 40);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, health) == 52);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, player_health_armour) == 56);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, siren) == 64);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, landing_gear) == 65);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, trailer_id) == 66);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, has_trailer) == 68);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, u14) == 69);
    assert!(std::mem::offset_of!(VehicleDriverSyncPacket, u15) == 72);
};

impl VirtualReturn for VehicleDriverSyncPacket {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `VehicleParams` in `Server/Components/Vehicles/vehicles.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleParams {
    /// `engine`.
    pub engine: i8,
    /// `lights`.
    pub lights: i8,
    /// `alarm`.
    pub alarm: i8,
    /// `doors`.
    pub doors: i8,
    /// `bonnet`.
    pub bonnet: i8,
    /// `boot`.
    pub boot: i8,
    /// `objective`.
    pub objective: i8,
    /// `siren`.
    pub siren: i8,
    /// `doorDriver`.
    pub door_driver: i8,
    /// `doorPassenger`.
    pub door_passenger: i8,
    /// `doorBackLeft`.
    pub door_back_left: i8,
    /// `doorBackRight`.
    pub door_back_right: i8,
    /// `windowDriver`.
    pub window_driver: i8,
    /// `windowPassenger`.
    pub window_passenger: i8,
    /// `windowBackLeft`.
    pub window_back_left: i8,
    /// `windowBackRight`.
    pub window_back_right: i8,
}

impl Default for VehicleParams {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            engine: -1,
            lights: -1,
            alarm: -1,
            doors: -1,
            bonnet: -1,
            boot: -1,
            objective: -1,
            siren: -1,
            door_driver: -1,
            door_passenger: -1,
            door_back_left: -1,
            door_back_right: -1,
            window_driver: -1,
            window_passenger: -1,
            window_back_left: -1,
            window_back_right: -1,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<VehicleParams>() == 16);
    assert!(std::mem::offset_of!(VehicleParams, engine) == 0);
    assert!(std::mem::offset_of!(VehicleParams, lights) == 1);
    assert!(std::mem::offset_of!(VehicleParams, alarm) == 2);
    assert!(std::mem::offset_of!(VehicleParams, doors) == 3);
    assert!(std::mem::offset_of!(VehicleParams, bonnet) == 4);
    assert!(std::mem::offset_of!(VehicleParams, boot) == 5);
    assert!(std::mem::offset_of!(VehicleParams, objective) == 6);
    assert!(std::mem::offset_of!(VehicleParams, siren) == 7);
    assert!(std::mem::offset_of!(VehicleParams, door_driver) == 8);
    assert!(std::mem::offset_of!(VehicleParams, door_passenger) == 9);
    assert!(std::mem::offset_of!(VehicleParams, door_back_left) == 10);
    assert!(std::mem::offset_of!(VehicleParams, door_back_right) == 11);
    assert!(std::mem::offset_of!(VehicleParams, window_driver) == 12);
    assert!(std::mem::offset_of!(VehicleParams, window_passenger) == 13);
    assert!(std::mem::offset_of!(VehicleParams, window_back_left) == 14);
    assert!(std::mem::offset_of!(VehicleParams, window_back_right) == 15);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<VehicleParams>() == 16);
    assert!(std::mem::offset_of!(VehicleParams, engine) == 0);
    assert!(std::mem::offset_of!(VehicleParams, lights) == 1);
    assert!(std::mem::offset_of!(VehicleParams, alarm) == 2);
    assert!(std::mem::offset_of!(VehicleParams, doors) == 3);
    assert!(std::mem::offset_of!(VehicleParams, bonnet) == 4);
    assert!(std::mem::offset_of!(VehicleParams, boot) == 5);
    assert!(std::mem::offset_of!(VehicleParams, objective) == 6);
    assert!(std::mem::offset_of!(VehicleParams, siren) == 7);
    assert!(std::mem::offset_of!(VehicleParams, door_driver) == 8);
    assert!(std::mem::offset_of!(VehicleParams, door_passenger) == 9);
    assert!(std::mem::offset_of!(VehicleParams, door_back_left) == 10);
    assert!(std::mem::offset_of!(VehicleParams, door_back_right) == 11);
    assert!(std::mem::offset_of!(VehicleParams, window_driver) == 12);
    assert!(std::mem::offset_of!(VehicleParams, window_passenger) == 13);
    assert!(std::mem::offset_of!(VehicleParams, window_back_left) == 14);
    assert!(std::mem::offset_of!(VehicleParams, window_back_right) == 15);
};

impl VirtualReturn for VehicleParams {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `VehiclePassengerSyncPacket` in `Server/Components/Vehicles/vehicles.hpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VehiclePassengerSyncPacket {
    /// `PlayerID`.
    pub player_id: i32,
    /// `VehicleID`.
    pub vehicle_id: i32,
    /// `(anonymous union)`.
    pub u2: VehiclePassengerSyncPacket2,
    /// `Keys`.
    pub keys: u16,
    /// `HealthArmour`.
    pub health_armour: Vector2,
    /// `LeftRight`.
    pub left_right: u16,
    /// `UpDown`.
    pub up_down: u16,
    /// `Position`.
    pub position: Vector3,
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<VehiclePassengerSyncPacket>() == 36);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, player_id) == 0);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, vehicle_id) == 4);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, u2) == 8);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, keys) == 10);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, health_armour) == 12);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, left_right) == 20);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, up_down) == 22);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, position) == 24);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<VehiclePassengerSyncPacket>() == 36);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, player_id) == 0);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, vehicle_id) == 4);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, u2) == 8);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, keys) == 10);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, health_armour) == 12);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, left_right) == 20);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, up_down) == 22);
    assert!(std::mem::offset_of!(VehiclePassengerSyncPacket, position) == 24);
};

impl VirtualReturn for VehiclePassengerSyncPacket {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `VehicleSpawnData` in `Server/Components/Vehicles/vehicles.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleSpawnData {
    /// `respawnDelay`.
    pub respawn_delay: Seconds,
    /// `modelID`.
    pub model_id: i32,
    /// `position`.
    pub position: Vector3,
    /// `zRotation`.
    pub z_rotation: f32,
    /// `colour1`.
    pub colour1: i32,
    /// `colour2`.
    pub colour2: i32,
    /// `siren`.
    pub siren: bool,
    /// `interior`.
    pub interior: i32,
}

impl Default for VehicleSpawnData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            respawn_delay: Seconds(0),
            model_id: 0,
            position: Vector3::ZERO,
            z_rotation: 0.0,
            colour1: 0,
            colour2: 0,
            siren: false,
            interior: 0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<VehicleSpawnData>() == 44);
    assert!(std::mem::offset_of!(VehicleSpawnData, respawn_delay) == 0);
    assert!(std::mem::offset_of!(VehicleSpawnData, model_id) == 8);
    assert!(std::mem::offset_of!(VehicleSpawnData, position) == 12);
    assert!(std::mem::offset_of!(VehicleSpawnData, z_rotation) == 24);
    assert!(std::mem::offset_of!(VehicleSpawnData, colour1) == 28);
    assert!(std::mem::offset_of!(VehicleSpawnData, colour2) == 32);
    assert!(std::mem::offset_of!(VehicleSpawnData, siren) == 36);
    assert!(std::mem::offset_of!(VehicleSpawnData, interior) == 40);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<VehicleSpawnData>() == 48);
    assert!(std::mem::offset_of!(VehicleSpawnData, respawn_delay) == 0);
    assert!(std::mem::offset_of!(VehicleSpawnData, model_id) == 8);
    assert!(std::mem::offset_of!(VehicleSpawnData, position) == 12);
    assert!(std::mem::offset_of!(VehicleSpawnData, z_rotation) == 24);
    assert!(std::mem::offset_of!(VehicleSpawnData, colour1) == 28);
    assert!(std::mem::offset_of!(VehicleSpawnData, colour2) == 32);
    assert!(std::mem::offset_of!(VehicleSpawnData, siren) == 36);
    assert!(std::mem::offset_of!(VehicleSpawnData, interior) == 40);
};

impl VirtualReturn for VehicleSpawnData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `VehicleTrailerSyncPacket` in `Server/Components/Vehicles/vehicles.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleTrailerSyncPacket {
    /// `VehicleID`.
    pub vehicle_id: i32,
    /// `PlayerID`.
    pub player_id: i32,
    /// `Position`.
    pub position: Vector3,
    /// `Quat`.
    pub quat: Vector4,
    /// `Velocity`.
    pub velocity: Vector3,
    /// `TurnVelocity`.
    pub turn_velocity: Vector3,
}

impl Default for VehicleTrailerSyncPacket {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            vehicle_id: 0,
            player_id: 0,
            position: Vector3::ZERO,
            quat: Vector4::ZERO,
            velocity: Vector3::ZERO,
            turn_velocity: Vector3::ZERO,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<VehicleTrailerSyncPacket>() == 60);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, vehicle_id) == 0);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, player_id) == 4);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, position) == 8);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, quat) == 20);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, velocity) == 36);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, turn_velocity) == 48);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<VehicleTrailerSyncPacket>() == 60);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, vehicle_id) == 0);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, player_id) == 4);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, position) == 8);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, quat) == 20);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, velocity) == 36);
    assert!(std::mem::offset_of!(VehicleTrailerSyncPacket, turn_velocity) == 48);
};

impl VirtualReturn for VehicleTrailerSyncPacket {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `VehicleUnoccupiedSyncPacket` in `Server/Components/Vehicles/vehicles.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleUnoccupiedSyncPacket {
    /// `VehicleID`.
    pub vehicle_id: i32,
    /// `PlayerID`.
    pub player_id: i32,
    /// `SeatID`.
    pub seat_id: u8,
    /// `Roll`.
    pub roll: Vector3,
    /// `Rotation`.
    pub rotation: Vector3,
    /// `Position`.
    pub position: Vector3,
    /// `Velocity`.
    pub velocity: Vector3,
    /// `AngularVelocity`.
    pub angular_velocity: Vector3,
    /// `Health`.
    pub health: f32,
}

impl Default for VehicleUnoccupiedSyncPacket {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self {
            vehicle_id: 0,
            player_id: 0,
            seat_id: 0,
            roll: Vector3::ZERO,
            rotation: Vector3::ZERO,
            position: Vector3::ZERO,
            velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
            health: 0.0,
        }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<VehicleUnoccupiedSyncPacket>() == 76);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, vehicle_id) == 0);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, player_id) == 4);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, seat_id) == 8);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, roll) == 12);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, rotation) == 24);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, position) == 36);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, velocity) == 48);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, angular_velocity) == 60);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, health) == 72);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<VehicleUnoccupiedSyncPacket>() == 76);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, vehicle_id) == 0);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, player_id) == 4);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, seat_id) == 8);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, roll) == 12);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, rotation) == 24);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, position) == 36);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, velocity) == 48);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, angular_velocity) == 60);
    assert!(std::mem::offset_of!(VehicleUnoccupiedSyncPacket, health) == 72);
};

impl VirtualReturn for VehicleUnoccupiedSyncPacket {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `WeaponSlotData` in `player.hpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSlotData {
    /// `id`.
    pub id: u8,
    /// `ammo`.
    pub ammo: u32,
}

impl Default for WeaponSlotData {
    /// The C++ struct's own defaults.
    fn default() -> Self {
        Self { id: 0, ammo: 0 }
    }
}

#[cfg(all(target_arch = "x86", not(target_env = "msvc")))]
const _: () = {
    assert!(std::mem::size_of::<WeaponSlotData>() == 8);
    assert!(std::mem::offset_of!(WeaponSlotData, id) == 0);
    assert!(std::mem::offset_of!(WeaponSlotData, ammo) == 4);
};

#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(std::mem::size_of::<WeaponSlotData>() == 8);
    assert!(std::mem::offset_of!(WeaponSlotData, id) == 0);
    assert!(std::mem::offset_of!(WeaponSlotData, ammo) == 4);
};

impl VirtualReturn for WeaponSlotData {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}
