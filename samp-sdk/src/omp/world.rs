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
use super::types::{Colour, StringView, UID, Vector2, Vector3};
use super::vtable::call_vtable;

/// UID of the Open Multiplayer `TextDraws` component.
pub const TEXTDRAWS_COMPONENT_UID: UID = 0x9b5d_c2b1_d15c_992a;

/// UID of the Open Multiplayer `GangZones` component.
pub const GANGZONES_COMPONENT_UID: UID = 0xb335_1d11_ee8d_8056;

/// UID of the Open Multiplayer `Actors` component.
pub const ACTORS_COMPONENT_UID: UID = 0xc81c_a021_eae2_ad5c;

/// UID of the Open Multiplayer `TextLabels` component.
pub const TEXTLABELS_COMPONENT_UID: UID = 0xa0c5_7ea8_0a00_9742;

/// UID of the Open Multiplayer `Menus` component.
pub const MENUS_COMPONENT_UID: UID = 0x621e_219e_b97e_e0b2;

/// UID of the Open Multiplayer `Classes` component.
pub const CLASSES_COMPONENT_UID: UID = 0x8cfb_3183_976d_a208;

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

/// `ITextLabelsComponent::create` — the global overload. Three of them share
/// the name (global, per player, per vehicle), so MSVC emits the set reversed:
/// [18], [17], [16] against Itanium's [18], [19], [20].
#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_TEXTLABEL: usize = 18;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_TEXTLABEL: usize = 18;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_MENU: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_MENU: usize = 17;

#[cfg(not(target_env = "msvc"))]
const SLOT_CREATE_CLASS: usize = 19;
#[cfg(target_env = "msvc")]
const SLOT_CREATE_CLASS: usize = 17;

/// How many weapon slots a spawn class carries (`MAX_WEAPON_SLOTS`).
pub const MAX_WEAPON_SLOTS: usize = 13;

/// One weapon slot of a spawn class (`WeaponSlotData`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeaponSlot {
    pub id: u8,
    pub ammo: u32,
}

/// Opaque handle for `ITextLabelsComponent*`.
#[repr(C)]
pub struct ITextLabelsComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `ITextLabel*`.
#[repr(C)]
pub struct ITextLabel {
    _opaque: [u8; 0],
}

/// Opaque handle for `IMenusComponent*`.
#[repr(C)]
pub struct IMenusComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `IMenu*`.
#[repr(C)]
pub struct IMenu {
    _opaque: [u8; 0],
}

/// Opaque handle for `IClassesComponent*`.
#[repr(C)]
pub struct IClassesComponent {
    _opaque: [u8; 0],
}

/// Opaque handle for `IClass*`.
#[repr(C)]
pub struct IClass {
    _opaque: [u8; 0],
}

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
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_OBJECT,
        (i32, Vector3, Vector3, f32) -> *mut super::players::IObject,
        (model, position, rotation, draw_distance),
        std::ptr::null_mut()
    )
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
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_PICKUP,
        (i32, PickupType, Vector3, u32, bool) -> *mut IPickup,
        (model, pickup_type, position, virtual_world, is_static),
        std::ptr::null_mut()
    )
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
    let text = StringView {
        data: text.as_ptr(),
        len: text.len(),
    };
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_TEXTDRAW,
        (Vector2, StringView) -> *mut ITextDraw,
        (position, text),
        std::ptr::null_mut()
    )
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
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_GANGZONE,
        (GangZonePos) -> *mut IGangZone,
        (area),
        std::ptr::null_mut()
    )
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
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_ACTOR,
        (i32, Vector3, f32) -> *mut IActor,
        (skin, position, angle),
        std::ptr::null_mut()
    )
}

/// Casts a component handle obtained by UID into the text labels component.
///
/// # Safety
/// `component` must be what `queryComponent(TEXTLABELS_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_textlabels_component(
    component: *mut ServerComponent,
) -> *mut ITextLabelsComponent {
    component.cast::<ITextLabelsComponent>()
}

/// Casts a component handle obtained by UID into the menus component.
///
/// # Safety
/// `component` must be what `queryComponent(MENUS_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_menus_component(component: *mut ServerComponent) -> *mut IMenusComponent {
    component.cast::<IMenusComponent>()
}

/// Casts a component handle obtained by UID into the classes component.
///
/// # Safety
/// `component` must be what `queryComponent(CLASSES_COMPONENT_UID)` returned.
#[must_use]
pub unsafe fn as_classes_component(component: *mut ServerComponent) -> *mut IClassesComponent {
    component.cast::<IClassesComponent>()
}

/// `ITextLabelsComponent::create(text, colour, pos, drawDist, vw, los)` — a
/// label everyone sees.
///
/// `line_of_sight` decides whether the label shows through walls.
///
/// # Safety
/// `component` must be a live `ITextLabelsComponent`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub unsafe fn create_textlabel(
    component: *mut ITextLabelsComponent,
    text: &str,
    colour: Colour,
    position: Vector3,
    draw_distance: f32,
    virtual_world: i32,
    line_of_sight: bool,
) -> *mut ITextLabel {
    let text = StringView {
        data: text.as_ptr(),
        len: text.len(),
    };
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_TEXTLABEL,
        (StringView, Colour, Vector3, f32, i32, bool) -> *mut ITextLabel,
        (text, colour, position, draw_distance, virtual_world, line_of_sight),
        std::ptr::null_mut()
    )
}

/// `IMenusComponent::create(title, position, columns, col1Width, col2Width)`.
///
/// # Safety
/// `component` must be a live `IMenusComponent`.
#[must_use]
pub unsafe fn create_menu(
    component: *mut IMenusComponent,
    title: &str,
    position: Vector2,
    columns: u8,
    column1_width: f32,
    column2_width: f32,
) -> *mut IMenu {
    let title = StringView {
        data: title.as_ptr(),
        len: title.len(),
    };
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_MENU,
        (StringView, Vector2, u8, f32, f32) -> *mut IMenu,
        (title, position, columns, column1_width, column2_width),
        std::ptr::null_mut()
    )
}

/// `IClassesComponent::create(skin, team, spawn, angle, weapons)` — a spawn
/// class, what `AddPlayerClass` creates on the script side.
///
/// The weapons array is passed by reference, so it only has to outlive the
/// call.
///
/// # Safety
/// `component` must be a live `IClassesComponent`.
#[must_use]
pub unsafe fn create_class(
    component: *mut IClassesComponent,
    skin: i32,
    team: i32,
    spawn: Vector3,
    angle: f32,
    weapons: &[WeaponSlot; MAX_WEAPON_SLOTS],
) -> *mut IClass {
    call_vtable!(
        component.cast::<u8>(),
        0,
        SLOT_CREATE_CLASS,
        (i32, i32, Vector3, f32, *const WeaponSlot) -> *mut IClass,
        (skin, team, spawn, angle, weapons.as_ptr()),
        std::ptr::null_mut()
    )
}

/// `IEntity::getID()` for any entity that carries the subobject — an object, a
/// pickup, a vehicle or a player.
///
/// # Safety
/// `entity` must point at an `IExtensible`-derived interface that also inherits
/// `IEntity`, which every entity in the SDK does.
#[must_use]
pub unsafe fn entity_id(entity: *mut u8) -> i32 {
    call_vtable!(entity, ENTITY_OFFSET, SLOT_ENTITY_GET_ID, () -> i32, (), -1)
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
    fn the_last_batch_of_uids_and_slots_match() {
        assert_eq!(TEXTLABELS_COMPONENT_UID, 0xa0c5_7ea8_0a00_9742);
        assert_eq!(MENUS_COMPONENT_UID, 0x621e_219e_b97e_e0b2);
        assert_eq!(CLASSES_COMPONENT_UID, 0x8cfb_3183_976d_a208);

        // Text labels overload `create` three ways; the global one happens to
        // land on 18 in both ABIs, which the reversal makes a coincidence
        // rather than a rule.
        #[cfg(not(target_env = "msvc"))]
        assert_eq!(
            [SLOT_CREATE_TEXTLABEL, SLOT_CREATE_MENU, SLOT_CREATE_CLASS],
            [18, 19, 19]
        );
        #[cfg(target_env = "msvc")]
        assert_eq!(
            [SLOT_CREATE_TEXTLABEL, SLOT_CREATE_MENU, SLOT_CREATE_CLASS],
            [18, 17, 17]
        );

        // `WeaponSlotData` is a byte and a word, and the server reads an array
        // of them.
        assert_eq!(
            std::mem::size_of::<WeaponSlot>(),
            8,
            "padded to 4-byte alignment"
        );
        assert_eq!(MAX_WEAPON_SLOTS, 13);
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
