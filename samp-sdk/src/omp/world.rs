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
use super::types::{UID, Vector3};

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
