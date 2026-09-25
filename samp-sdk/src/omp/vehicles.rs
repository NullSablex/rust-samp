//! The Open Multiplayer vehicle component.
//!
//! `IVehiclesComponent` is queried like any other component, by UID. From it a
//! plugin creates vehicles and reaches the vehicle event dispatcher; each
//! `IVehicle` answers for its own model and health, and — through the `IEntity`
//! subobject it carries, exactly like a player — for its position.
//!
//! ## Slots, per ABI
//!
//! | Method | Itanium | MSVC |
//! | ------ | :-----: | :--: |
//! | `IVehiclesComponent::create(bool, int, Vector3, …)` | 19 | **18** |
//! | `IVehiclesComponent::getEventDispatcher` | 21 | 19 |
//! | `IVehicle::setColour` | 11 | 10 |
//! | `IVehicle::setHealth` | 13 | 12 |
//! | `IVehicle::getHealth` | 14 | 13 |
//! | `IVehicle::getModel` | 57 | 56 |
//!
//! `create` is overloaded — the other one takes a `VehicleSpawnData` — so MSVC
//! emits the pair in reverse: the eight-argument overload the SDK calls lands
//! at [18] there, with [17] holding the other one. That is the same trap the
//! timer component sprang in v3.5.0, and `scripts/omp-vtable.py` reports it
//! without anyone having to remember the rule.

use super::players::{ENTITY_OFFSET, SLOT_ENTITY_GET_POSITION};
use super::server::ServerComponent;
use super::types::{UID, Vector3};

/// UID of the Open Multiplayer `Vehicles` component.
pub const VEHICLES_COMPONENT_UID: UID = 0x3f1f_62ee_9e22_ab19;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_VEHICLE: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_VEHICLE: usize = 18;

#[cfg(not(target_env = "msvc"))]
const SLOT_VEHICLE_SET_COLOUR: usize = 11;
#[cfg(target_env = "msvc")]
const SLOT_VEHICLE_SET_COLOUR: usize = 10;

#[cfg(not(target_env = "msvc"))]
const SLOT_VEHICLE_SET_HEALTH: usize = 13;
#[cfg(target_env = "msvc")]
const SLOT_VEHICLE_SET_HEALTH: usize = 12;

#[cfg(not(target_env = "msvc"))]
const SLOT_VEHICLE_GET_HEALTH: usize = 14;
#[cfg(target_env = "msvc")]
const SLOT_VEHICLE_GET_HEALTH: usize = 13;

#[cfg(not(target_env = "msvc"))]
const SLOT_VEHICLE_GET_MODEL: usize = 57;
#[cfg(target_env = "msvc")]
const SLOT_VEHICLE_GET_MODEL: usize = 56;

/// Opaque handle for the server's `IVehiclesComponent*`.
#[repr(C)]
pub struct IVehiclesComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for the server's `IVehicle*`.
#[repr(C)]
pub struct IVehicle {
    _opaque: [u8; 0],
}

/// Casts a component handle obtained by UID into the vehicles component.
///
/// # Safety
/// `component` must be what `queryComponent(VEHICLES_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_vehicles_component(component: *mut ServerComponent) -> *mut IVehiclesComponent {
    component.cast::<IVehiclesComponent>()
}

/// `IVehiclesComponent::create(...)` — spawns a vehicle.
///
/// `respawn_delay` is in seconds; a negative value means "never respawn", which
/// is the server's own default. `colour1`/`colour2` of `-1` ask the server to
/// pick from the model's palette.
///
/// Returns null when the component pointer is unusable or the server refuses
/// (an unknown model, or the vehicle pool being full).
///
/// # Safety
/// `component` must be a live `IVehiclesComponent`.
#[must_use]
// Mirrors `IVehiclesComponent::create` argument for argument. Grouping them
// into a struct would read better in isolation and worse here: the call has to
// be checked against the C++ declaration, and a one-to-one mapping is what
// makes that check possible.
#[allow(clippy::too_many_arguments)]
pub unsafe fn create_vehicle(
    component: *mut IVehiclesComponent,
    model: i32,
    position: Vector3,
    z_angle: f32,
    colour1: i32,
    colour2: i32,
    respawn_delay_secs: i64,
    siren: bool,
) -> *mut IVehicle {
    #[cfg(not(target_env = "msvc"))]
    type CreateFn = unsafe extern "C" fn(
        *mut u8,
        bool,
        i32,
        Vector3,
        f32,
        i32,
        i32,
        i64,
        bool,
    ) -> *mut IVehicle;
    #[cfg(target_env = "msvc")]
    type CreateFn = unsafe extern "thiscall" fn(
        *mut u8,
        bool,
        i32,
        Vector3,
        f32,
        i32,
        i32,
        i64,
        bool,
    ) -> *mut IVehicle;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(component.cast::<u8>(), 0, SLOT_CREATE_VEHICLE)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe {
        create(
            this,
            false, // isStatic
            model,
            position,
            z_angle,
            colour1,
            colour2,
            respawn_delay_secs,
            siren,
        )
    }
}

/// Reads a `f32` getter that takes no arguments from the vehicle's vtable.
unsafe fn vehicle_f32(vehicle: *mut IVehicle, slot: usize) -> f32 {
    #[cfg(not(target_env = "msvc"))]
    type GetFn = unsafe extern "C" fn(*mut u8) -> f32;
    #[cfg(target_env = "msvc")]
    type GetFn = unsafe extern "thiscall" fn(*mut u8) -> f32;

    let Some((this, f_ptr)) =
        (unsafe { super::vtable::secondary_call_target_ptr(vehicle.cast::<u8>(), 0, slot) })
    else {
        return 0.0;
    };
    let get: GetFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get(this) }
}

/// `IVehicle::getModel()`.
///
/// # Safety
/// `vehicle` must come from [`create_vehicle`] and still exist.
#[must_use]
pub unsafe fn vehicle_model(vehicle: *mut IVehicle) -> i32 {
    #[cfg(not(target_env = "msvc"))]
    type GetModelFn = unsafe extern "C" fn(*mut u8) -> i32;
    #[cfg(target_env = "msvc")]
    type GetModelFn = unsafe extern "thiscall" fn(*mut u8) -> i32;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(vehicle.cast::<u8>(), 0, SLOT_VEHICLE_GET_MODEL)
    }) else {
        return 0;
    };
    let get_model: GetModelFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_model(this) }
}

/// `IVehicle::getHealth()`.
///
/// # Safety
/// See [`vehicle_model`].
#[must_use]
pub unsafe fn vehicle_health(vehicle: *mut IVehicle) -> f32 {
    unsafe { vehicle_f32(vehicle, SLOT_VEHICLE_GET_HEALTH) }
}

/// `IVehicle::setHealth(float)`.
///
/// # Safety
/// See [`vehicle_model`].
pub unsafe fn vehicle_set_health(vehicle: *mut IVehicle, health: f32) {
    #[cfg(not(target_env = "msvc"))]
    type SetHealthFn = unsafe extern "C" fn(*mut u8, f32);
    #[cfg(target_env = "msvc")]
    type SetHealthFn = unsafe extern "thiscall" fn(*mut u8, f32);

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(vehicle.cast::<u8>(), 0, SLOT_VEHICLE_SET_HEALTH)
    }) else {
        return;
    };
    let set_health: SetHealthFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { set_health(this, health) };
}

/// `IVehicle::setColour(int, int)`.
///
/// # Safety
/// See [`vehicle_model`].
pub unsafe fn vehicle_set_colour(vehicle: *mut IVehicle, colour1: i32, colour2: i32) {
    #[cfg(not(target_env = "msvc"))]
    type SetColourFn = unsafe extern "C" fn(*mut u8, i32, i32);
    #[cfg(target_env = "msvc")]
    type SetColourFn = unsafe extern "thiscall" fn(*mut u8, i32, i32);

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(vehicle.cast::<u8>(), 0, SLOT_VEHICLE_SET_COLOUR)
    }) else {
        return;
    };
    let set_colour: SetColourFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { set_colour(this, colour1, colour2) };
}

/// `IEntity::getPosition()` for a vehicle.
///
/// `IVehicle` carries the same `IEntity` subobject a player does, at the same
/// offset, so this is the player accessor with a different handle.
///
/// # Safety
/// See [`vehicle_model`].
#[must_use]
pub unsafe fn vehicle_position(vehicle: *mut IVehicle) -> Vector3 {
    #[cfg(not(target_env = "msvc"))]
    type GetPositionFn = unsafe extern "C" fn(*mut u8) -> Vector3;
    #[cfg(target_env = "msvc")]
    type GetPositionFn = unsafe extern "thiscall" fn(*mut u8) -> Vector3;

    let zero = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(
            vehicle.cast::<u8>(),
            ENTITY_OFFSET,
            SLOT_ENTITY_GET_POSITION,
        )
    }) else {
        return zero;
    };
    let get_position: GetPositionFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_position(this) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_matches_the_header() {
        // `VehicleComponent_UID` in `vehicles.hpp`.
        assert_eq!(VEHICLES_COMPONENT_UID, 0x3f1f_62ee_9e22_ab19);
    }

    #[test]
    fn slots_match_what_clang_reports() {
        // `scripts/omp-vtable.py IVehiclesComponent` / `IVehicle`. The `create`
        // pair is overloaded, so MSVC emits it reversed: [18] is the
        // eight-argument overload there, [17] the `VehicleSpawnData` one.
        #[cfg(not(target_env = "msvc"))]
        let expected = [19, 11, 13, 14, 57];
        #[cfg(target_env = "msvc")]
        let expected = [18, 10, 12, 13, 56];
        assert_eq!(
            [
                SLOT_CREATE_VEHICLE,
                SLOT_VEHICLE_SET_COLOUR,
                SLOT_VEHICLE_SET_HEALTH,
                SLOT_VEHICLE_GET_HEALTH,
                SLOT_VEHICLE_GET_MODEL,
            ],
            expected
        );
    }

    #[test]
    fn a_null_component_creates_nothing() {
        let position = Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let vehicle =
            unsafe { create_vehicle(std::ptr::null_mut(), 411, position, 0.0, -1, -1, -1, false) };
        assert!(vehicle.is_null());
    }
}
