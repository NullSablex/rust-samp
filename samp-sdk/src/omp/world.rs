//! World entities: objects and pickups.
//!
//! Both follow the shape the [`vehicles`](super::vehicles) module established —
//! query the component by UID, call `create`, and talk to the handle that comes
//! back. Position comes from the shared `IEntity` subobject, so the accessors
//! are the same ones a player or a vehicle uses.
//!
//! | Method | Itanium | MSVC |
//! | ------ | :-----: | :--: |
//! | `IObjectsComponent::create(int, Vector3, Vector3, float)` | 21 | 19 |
//! | `IPickupsComponent::create(int, PickupType, Vector3, uint32, bool)` | 19 | 17 |
//!
//! Slots from `scripts/omp-vtable.py`; both components declare overloads (a
//! player-scoped `create` among them), so the MSVC column is not simply one
//! less.

use super::players::{ENTITY_OFFSET, SLOT_ENTITY_GET_ID};
use super::server::ServerComponent;
use super::types::{StringView, UID, Vector2, Vector3};

/// UID of the Open Multiplayer `TextDraws` component.
pub const TEXTDRAWS_COMPONENT_UID: UID = 0x9b5d_c2b1_d15c_992a;

/// UID of the Open Multiplayer `GangZones` component.
pub const GANGZONES_COMPONENT_UID: UID = 0xb335_1d11_ee8d_8056;

/// UID of the Open Multiplayer `Actors` component.
pub const ACTORS_COMPONENT_UID: UID = 0xc81c_a021_eae2_ad5c;

/// UID of the Open Multiplayer `Objects` component.
pub const OBJECTS_COMPONENT_UID: UID = 0x59f8_415f_72da_6160;

/// UID of the Open Multiplayer `Pickups` component.
pub const PICKUPS_COMPONENT_UID: UID = 0xcf30_4faa_363d_d971;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_OBJECT: usize = 21;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_OBJECT: usize = 19;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_PICKUP: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_PICKUP: usize = 17;

/// `ITextDrawsComponent::create(Vector2, StringView)` — the text overload,
/// which MSVC emits after the model one ([17] against [18]).
#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_TEXTDRAW: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_TEXTDRAW: usize = 18;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_GANGZONE: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_GANGZONE: usize = 17;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_ACTOR: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_ACTOR: usize = 17;

/// Opaque handle for `ITextDrawsComponent*`.
#[repr(C)]
pub struct ITextDrawsComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `ITextDraw*`.
#[repr(C)]
pub struct ITextDraw {
    _opaque: [u8; 0],
}

/// Opaque handle for `IGangZonesComponent*`.
#[repr(C)]
pub struct IGangZonesComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `IGangZone*`.
#[repr(C)]
pub struct IGangZone {
    _opaque: [u8; 0],
}

/// Opaque handle for `IActorsComponent*`.
#[repr(C)]
pub struct IActorsComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `IActor*`.
#[repr(C)]
pub struct IActor {
    _opaque: [u8; 0],
}

/// The rectangle a gang zone covers, as `GangZonePos` declares it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GangZonePos {
    pub min: Vector2,
    pub max: Vector2,
}

/// Opaque handle for `IObjectsComponent*`.
#[repr(C)]
pub struct IObjectsComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `IPickupsComponent*`.
#[repr(C)]
pub struct IPickupsComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `IPickup*`.
#[repr(C)]
pub struct IPickup {
    _opaque: [u8; 0],
}

/// What a pickup does when a player walks into it (`PickupType` in
/// `pickups.hpp`). The values the scripts use are passed straight through, so
/// this is an `i32` rather than an enum that would have to track the server's.
pub type PickupType = i32;

/// Casts a component handle obtained by UID into the objects component.
///
/// # Safety
/// `component` must be what `queryComponent(OBJECTS_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_objects_component(component: *mut ServerComponent) -> *mut IObjectsComponent {
    component.cast::<IObjectsComponent>()
}

/// Casts a component handle obtained by UID into the pickups component.
///
/// # Safety
/// `component` must be what `queryComponent(PICKUPS_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_pickups_component(component: *mut ServerComponent) -> *mut IPickupsComponent {
    component.cast::<IPickupsComponent>()
}

/// `IObjectsComponent::create(modelID, position, rotation, drawDistance)`.
///
/// A `draw_distance` of `0.0` asks the server for its default. Returns null
/// when the object pool is full.
///
/// # Safety
/// `component` must be a live `IObjectsComponent`.
#[must_use]
pub unsafe fn create_object(
    component: *mut IObjectsComponent,
    model: i32,
    position: Vector3,
    rotation: Vector3,
    draw_distance: f32,
) -> *mut super::players::IObject {
    #[cfg(not(target_env = "msvc"))]
    type CreateFn =
        unsafe extern "C" fn(*mut u8, i32, Vector3, Vector3, f32) -> *mut super::players::IObject;
    #[cfg(target_env = "msvc")]
    type CreateFn = unsafe extern "thiscall" fn(
        *mut u8,
        i32,
        Vector3,
        Vector3,
        f32,
    ) -> *mut super::players::IObject;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(component.cast::<u8>(), 0, SLOT_CREATE_OBJECT)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { create(this, model, position, rotation, draw_distance) }
}

/// `IPickupsComponent::create(modelId, type, pos, virtualWorld, isStatic)`.
///
/// # Safety
/// `component` must be a live `IPickupsComponent`.
#[must_use]
pub unsafe fn create_pickup(
    component: *mut IPickupsComponent,
    model: i32,
    pickup_type: PickupType,
    position: Vector3,
    virtual_world: u32,
    is_static: bool,
) -> *mut IPickup {
    #[cfg(not(target_env = "msvc"))]
    type CreateFn =
        unsafe extern "C" fn(*mut u8, i32, PickupType, Vector3, u32, bool) -> *mut IPickup;
    #[cfg(target_env = "msvc")]
    type CreateFn =
        unsafe extern "thiscall" fn(*mut u8, i32, PickupType, Vector3, u32, bool) -> *mut IPickup;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(component.cast::<u8>(), 0, SLOT_CREATE_PICKUP)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { create(this, model, pickup_type, position, virtual_world, is_static) }
}

/// Casts a component handle obtained by UID into the text draws component.
///
/// # Safety
/// `component` must be what `queryComponent(TEXTDRAWS_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_textdraws_component(component: *mut ServerComponent) -> *mut ITextDrawsComponent {
    component.cast::<ITextDrawsComponent>()
}

/// Casts a component handle obtained by UID into the gang zones component.
///
/// # Safety
/// `component` must be what `queryComponent(GANGZONES_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_gangzones_component(component: *mut ServerComponent) -> *mut IGangZonesComponent {
    component.cast::<IGangZonesComponent>()
}

/// Casts a component handle obtained by UID into the actors component.
///
/// # Safety
/// `component` must be what `queryComponent(ACTORS_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_actors_component(component: *mut ServerComponent) -> *mut IActorsComponent {
    component.cast::<IActorsComponent>()
}

/// `ITextDrawsComponent::create(position, text)` — a global text draw.
///
/// The text is borrowed for the call; the server copies it.
///
/// # Safety
/// `component` must be a live `ITextDrawsComponent`.
#[must_use]
pub unsafe fn create_textdraw(
    component: *mut ITextDrawsComponent,
    position: Vector2,
    text: &str,
) -> *mut ITextDraw {
    #[cfg(not(target_env = "msvc"))]
    type CreateFn = unsafe extern "C" fn(*mut u8, Vector2, StringView) -> *mut ITextDraw;
    #[cfg(target_env = "msvc")]
    type CreateFn = unsafe extern "thiscall" fn(*mut u8, Vector2, StringView) -> *mut ITextDraw;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(component.cast::<u8>(), 0, SLOT_CREATE_TEXTDRAW)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(f_ptr) };
    let view = StringView {
        data: text.as_ptr(),
        len: text.len(),
    };
    unsafe { create(this, position, view) }
}

/// `IGangZonesComponent::create(pos)`.
///
/// # Safety
/// `component` must be a live `IGangZonesComponent`.
#[must_use]
pub unsafe fn create_gangzone(
    component: *mut IGangZonesComponent,
    area: GangZonePos,
) -> *mut IGangZone {
    #[cfg(not(target_env = "msvc"))]
    type CreateFn = unsafe extern "C" fn(*mut u8, GangZonePos) -> *mut IGangZone;
    #[cfg(target_env = "msvc")]
    type CreateFn = unsafe extern "thiscall" fn(*mut u8, GangZonePos) -> *mut IGangZone;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(component.cast::<u8>(), 0, SLOT_CREATE_GANGZONE)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { create(this, area) }
}

/// `IActorsComponent::create(skin, pos, angle)` — a static NPC-looking actor.
///
/// # Safety
/// `component` must be a live `IActorsComponent`.
#[must_use]
pub unsafe fn create_actor(
    component: *mut IActorsComponent,
    skin: i32,
    position: Vector3,
    angle: f32,
) -> *mut IActor {
    #[cfg(not(target_env = "msvc"))]
    type CreateFn = unsafe extern "C" fn(*mut u8, i32, Vector3, f32) -> *mut IActor;
    #[cfg(target_env = "msvc")]
    type CreateFn = unsafe extern "thiscall" fn(*mut u8, i32, Vector3, f32) -> *mut IActor;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(component.cast::<u8>(), 0, SLOT_CREATE_ACTOR)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { create(this, skin, position, angle) }
}

/// `IEntity::getID()` for any entity that carries the subobject — an object, a
/// pickup, a vehicle or a player.
///
/// # Safety
/// `entity` must point at an `IExtensible`-derived interface that also inherits
/// `IEntity`, which every entity in the SDK does.
#[must_use]
pub unsafe fn entity_id(entity: *mut u8) -> i32 {
    #[cfg(not(target_env = "msvc"))]
    type GetIdFn = unsafe extern "C" fn(*mut u8) -> i32;
    #[cfg(target_env = "msvc")]
    type GetIdFn = unsafe extern "thiscall" fn(*mut u8) -> i32;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(entity, ENTITY_OFFSET, SLOT_ENTITY_GET_ID)
    }) else {
        return -1;
    };
    let get_id: GetIdFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_id(this) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uids_match_the_headers() {
        assert_eq!(OBJECTS_COMPONENT_UID, 0x59f8_415f_72da_6160);
        assert_eq!(PICKUPS_COMPONENT_UID, 0xcf30_4faa_363d_d971);
    }

    #[test]
    fn slots_match_what_clang_reports() {
        #[cfg(not(target_env = "msvc"))]
        assert_eq!([SLOT_CREATE_OBJECT, SLOT_CREATE_PICKUP], [21, 19]);
        #[cfg(target_env = "msvc")]
        assert_eq!([SLOT_CREATE_OBJECT, SLOT_CREATE_PICKUP], [19, 17]);
    }

    #[test]
    fn the_other_uids_and_slots_match() {
        assert_eq!(TEXTDRAWS_COMPONENT_UID, 0x9b5d_c2b1_d15c_992a);
        assert_eq!(GANGZONES_COMPONENT_UID, 0xb335_1d11_ee8d_8056);
        assert_eq!(ACTORS_COMPONENT_UID, 0xc81c_a021_eae2_ad5c);

        // Text draws overload `create`, so MSVC reverses that pair; the other
        // two do not, and lose only the destructor slot.
        #[cfg(not(target_env = "msvc"))]
        assert_eq!(
            [
                SLOT_CREATE_TEXTDRAW,
                SLOT_CREATE_GANGZONE,
                SLOT_CREATE_ACTOR
            ],
            [19, 19, 19]
        );
        #[cfg(target_env = "msvc")]
        assert_eq!(
            [
                SLOT_CREATE_TEXTDRAW,
                SLOT_CREATE_GANGZONE,
                SLOT_CREATE_ACTOR
            ],
            [18, 17, 17]
        );
    }

    #[test]
    fn null_components_create_nothing() {
        let zero = Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        assert!(unsafe { create_object(std::ptr::null_mut(), 1337, zero, zero, 0.0) }.is_null());
        assert!(unsafe { create_pickup(std::ptr::null_mut(), 1274, 1, zero, 0, true) }.is_null());
    }
}
