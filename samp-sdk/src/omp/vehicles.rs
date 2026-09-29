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
//! timer component sprang in v3.5.0, and `cargo xtask vtable` reports it
//! without anyone having to remember the rule.

use super::component_api::ComponentInterface;
use super::players::{ENTITY_OFFSET, SLOT_ENTITY_GET_POSITION};
use super::server::ServerComponent;
use super::types::{UID, Vector3};
use super::vtable::{call_vtable, opaque, slots, virtual_fns};

/// UID of the Open Multiplayer `Vehicles` component.
pub const VEHICLES_COMPONENT_UID: UID = 0x3f1f_62ee_9e22_ab19;

slots! {
    SLOT_CREATE_VEHICLE: usize = 19, 18;
    SLOT_VEHICLE_SET_COLOUR: usize = 11, 10;
    SLOT_VEHICLE_SET_HEALTH: usize = 13, 12;
    SLOT_VEHICLE_GET_HEALTH: usize = 14, 13;
    SLOT_VEHICLE_GET_MODEL: usize = 57, 56;
    /// Slot of `IVehiclesComponent::getEventDispatcher()`.
    SLOT_VEHICLE_DISPATCHER: usize = 21, 19;
    /// Offset of the `IReadOnlyPool<IVehicle>` subobject inside the component.
    ///
    /// Larger than the player pool's because `IVehiclesComponent` reaches it
    /// through `IComponent`, which carries an `IUIDProvider` of its own. From
    /// clang's record layout.
    VEHICLE_POOL_OFFSET: isize = 44, 64;
}

opaque! {
    /// Opaque handle for the server's `IVehiclesComponent*`.
    pub IVehiclesComponent;
    /// Opaque handle for the server's `IVehicle*`.
    pub IVehicle;
    /// Opaque handle for `IEventDispatcher<VehicleEventHandler>*`.
    pub IVehicleDispatcher;
}

/// `VehicleEventHandler` vtable — Itanium ABI.
///
/// Fourteen slots, in the order `vehicles.hpp` declares them. No virtual
/// destructor, so MSVC numbers them the same; only the calling convention
/// differs. The handlers returning `bool` can refuse the action: a `false` from
/// `on_vehicle_paint_job`, `on_vehicle_mod` or `on_vehicle_respray` rejects it.
#[cfg(not(target_env = "msvc"))]
#[repr(C)]
pub struct VehicleHandlerVTable {
    pub on_vehicle_stream_in: unsafe extern "C" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_vehicle_stream_out: unsafe extern "C" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_vehicle_death: unsafe extern "C" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_player_enter_vehicle:
        unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, bool),
    pub on_player_exit_vehicle: unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle),
    pub on_vehicle_damage_status_update:
        unsafe extern "C" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_vehicle_paint_job:
        unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, i32) -> bool,
    pub on_vehicle_mod:
        unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, i32) -> bool,
    pub on_vehicle_respray:
        unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, i32, i32) -> bool,
    pub on_enter_exit_mod_shop: unsafe extern "C" fn(*mut VehicleHandler, *mut u8, bool, i32),
    pub on_vehicle_spawn: unsafe extern "C" fn(*mut VehicleHandler, *mut IVehicle),
    pub on_unoccupied_vehicle_update:
        unsafe extern "C" fn(*mut VehicleHandler, *mut IVehicle, *mut u8, *const u8) -> bool,
    pub on_trailer_update:
        unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle) -> bool,
    pub on_vehicle_siren_state_change:
        unsafe extern "C" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, u8) -> bool,
}

/// `VehicleEventHandler` vtable — MSVC ABI (`this` in ECX).
#[cfg(target_env = "msvc")]
#[repr(C)]
pub struct VehicleHandlerVTable {
    pub on_vehicle_stream_in:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_vehicle_stream_out:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_vehicle_death: unsafe extern "thiscall" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_player_enter_vehicle:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, bool),
    pub on_player_exit_vehicle:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle),
    pub on_vehicle_damage_status_update:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut IVehicle, *mut u8),
    pub on_vehicle_paint_job:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, i32) -> bool,
    pub on_vehicle_mod:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, i32) -> bool,
    pub on_vehicle_respray:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, i32, i32) -> bool,
    pub on_enter_exit_mod_shop:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, bool, i32),
    pub on_vehicle_spawn: unsafe extern "thiscall" fn(*mut VehicleHandler, *mut IVehicle),
    pub on_unoccupied_vehicle_update:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut IVehicle, *mut u8, *const u8) -> bool,
    pub on_trailer_update:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle) -> bool,
    pub on_vehicle_siren_state_change:
        unsafe extern "thiscall" fn(*mut VehicleHandler, *mut u8, *mut IVehicle, u8) -> bool,
}

/// Object the server calls on vehicle events. The `*mut u8` arguments are
/// `IPlayer*`; casting them to [`super::players::IPlayer`] is the caller's
/// call, and keeps this module from depending on the player one.
#[repr(C)]
pub struct VehicleHandler {
    vtable: *const VehicleHandlerVTable,
}

// SAFETY: the handler is only ever touched on the server's main thread.
unsafe impl Send for VehicleHandler {}
unsafe impl Sync for VehicleHandler {}

impl VehicleHandler {
    /// Builds a handler backed by `vtable`.
    #[must_use]
    pub fn new(vtable: *const VehicleHandlerVTable) -> Self {
        Self { vtable }
    }
}

virtual_fns! {
    /// `IVehiclesComponent::getEventDispatcher()`.
    ///
    /// # Safety
    /// `component` must be a live `IVehiclesComponent`.
    #[must_use]
    pub fn vehicle_event_dispatcher(component: IVehiclesComponent) -> *mut IVehicleDispatcher = [0, SLOT_VEHICLE_DISPATCHER] or std::ptr::null_mut();
}

/// Registers `handler` on the vehicle dispatcher (`addEventHandler`, slot [0]).
///
/// # Safety
/// Both pointers must be valid, and `handler` must outlive the registration.
pub unsafe fn add_vehicle_handler(
    dispatcher: *mut IVehicleDispatcher,
    handler: *mut VehicleHandler,
) -> bool {
    call_vtable!(
        dispatcher.cast::<u8>(),
        0,
        0,
        (*mut VehicleHandler, i8) -> bool,
        (handler, 0),
        false
    )
}

virtual_fns! {
    /// `IReadOnlyPool<IVehicle>::get(int)` — the vehicle with that id, or null.
    ///
    /// # Safety
    /// `component` must be a live `IVehiclesComponent`.
    #[must_use]
    pub fn vehicle_by_id(component: IVehiclesComponent, id: i32) -> *mut IVehicle = [VEHICLE_POOL_OFFSET, super::players::SLOT_POOL_GET] or std::ptr::null_mut();

    /// `IEntity::getID()` for a vehicle — the id Pawn scripts use.
    ///
    /// # Safety
    /// See [`vehicle_model`].
    #[must_use]
    pub fn vehicle_id(vehicle: IVehicle) -> i32 = [ENTITY_OFFSET, super::players::SLOT_ENTITY_GET_ID] or -1;
}

/// Casts a component handle obtained by UID into the vehicles component.
///
/// # Safety
/// `component` must be what `queryComponent(VEHICLES_COMPONENT_UID)` returned.
#[must_use]
#[deprecated(
    since = "3.6.0",
    note = "the UID and the cast can disagree; use `omp_query::<Component<IVehiclesComponent>>()` and `Component::as_ptr`"
)]
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
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_VEHICLE,
        (bool, i32, Vector3, f32, i32, i32, i64, bool) -> *mut IVehicle,
        (false, model, position, z_angle, colour1, colour2, respawn_delay_secs, siren),
        std::ptr::null_mut()
    )
}

virtual_fns! {
    /// `IVehicle::getModel()`.
    ///
    /// # Safety
    /// `vehicle` must come from [`create_vehicle`] and still exist.
    #[must_use]
    pub fn vehicle_model(vehicle: IVehicle) -> i32 = [0, SLOT_VEHICLE_GET_MODEL] or 0;

    /// `IVehicle::getHealth()`.
    ///
    /// # Safety
    /// See [`vehicle_model`].
    #[must_use]
    pub fn vehicle_health(vehicle: IVehicle) -> f32 = [0, SLOT_VEHICLE_GET_HEALTH] or 0.0;

    /// `IVehicle::setHealth(float)`.
    ///
    /// # Safety
    /// See [`vehicle_model`].
    pub fn vehicle_set_health(vehicle: IVehicle, health: f32) = [0, SLOT_VEHICLE_SET_HEALTH];

    /// `IVehicle::setColour(int, int)`.
    ///
    /// # Safety
    /// See [`vehicle_model`].
    pub fn vehicle_set_colour(vehicle: IVehicle, colour1: i32, colour2: i32) = [0, SLOT_VEHICLE_SET_COLOUR];

    /// `IEntity::getPosition()` for a vehicle.
    ///
    /// `IVehicle` carries the same `IEntity` subobject a player does, at the same
    /// offset, so this is the player accessor with a different handle.
    ///
    /// # Safety
    /// See [`vehicle_model`].
    #[must_use]
    pub fn vehicle_position(vehicle: IVehicle) -> Vector3 = [ENTITY_OFFSET, SLOT_ENTITY_GET_POSITION] or Vector3::ZERO;
}

// Each interface knows its own UID, so `omp_query::<Component<I>>()` finds it.

impl ComponentInterface for IVehiclesComponent {
    const UID: UID = VEHICLES_COMPONENT_UID;
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
        // `cargo xtask vtable IVehiclesComponent` / `IVehicle`. The `create`
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
        let position = Vector3::ZERO;
        let vehicle =
            unsafe { create_vehicle(std::ptr::null_mut(), 411, position, 0.0, -1, -1, -1, false) };
        assert!(vehicle.is_null());
    }
}
