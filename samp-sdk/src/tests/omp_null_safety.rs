//! Every public entry point of the open.mp layer, called with a null handle.
//!
//! These wrappers take raw pointers the server owns. A plugin that asks for a
//! player who just disconnected, or queries a component the server did not
//! load, ends up calling them with null — so "null in, documented default out"
//! is part of the contract rather than an accident, and this is where that is
//! checked.
//!
//! What it does *not* check is a wrong-but-non-null pointer: nothing can, from
//! this side. That is what the slot constants, `scripts/check-abi-slots.py` and
//! the runs against real servers are for.

use crate::omp::players::*;
use crate::omp::types::{Colour, Vector2, Vector3};
use crate::omp::vehicles::*;
use crate::omp::world::*;

const ZERO: Vector3 = Vector3 {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

fn null<T>() -> *mut T {
    std::ptr::null_mut()
}

#[test]
fn player_readers_answer_without_dereferencing_null() {
    unsafe {
        assert_eq!(player_name(null()), None);
        assert!(!player_is_bot(null()));
        assert_eq!(player_id(null()), -1);
        assert_eq!(player_health(null()), 0.0);
        assert_eq!(player_armour(null()), 0.0);
        assert_eq!(player_score(null()), 0);
        assert_eq!(player_team(null()), 0);
        assert_eq!(player_money(null()), 0);
        assert_eq!(player_skin(null()), 0);
        assert_eq!(player_interior(null()), 0);
        assert_eq!(player_wanted_level(null()), 0);
        assert_eq!(player_virtual_world(null()), 0);
        assert_eq!(player_position(null()), ZERO);
    }
}

#[test]
fn player_writers_do_nothing_on_null() {
    // They return nothing, so what is being asserted is that the call returns
    // at all rather than following a null vtable.
    unsafe {
        player_kick(null());
        player_set_health(null(), 50.0);
        player_set_armour(null(), 50.0);
        player_set_score(null(), 1);
        player_set_team(null(), 1);
        player_set_money(null(), 1);
        player_give_money(null(), 1);
        player_set_skin(null(), 1, true);
        player_set_interior(null(), 1);
        player_set_wanted_level(null(), 1);
        player_set_weather(null(), 1);
        player_set_drunk_level(null(), 1);
        player_set_controllable(null(), false);
        player_set_position(null(), ZERO);
        player_set_virtual_world(null(), 1);
        player_send_message(null(), Colour::rgba(255, 255, 255, 255), "ignored");
    }
}

#[test]
fn pools_and_dispatchers_are_empty_without_a_server() {
    unsafe {
        assert!(player_pool(null()).is_null());
        assert!(player_by_id(null(), 0).is_null());
        assert!(all_players(null()).is_empty());

        // `first > last` is the empty range, so a caller looping over it stops
        // immediately instead of reading id zero.
        let bounds = pool_bounds(null(), 0);
        assert!(bounds.first > bounds.last);

        assert!(player_connect_dispatcher(null()).is_null());
        assert!(player_spawn_dispatcher(null()).is_null());
        assert!(player_text_dispatcher(null()).is_null());
        assert!(player_damage_dispatcher(null()).is_null());
        assert!(player_stream_dispatcher(null()).is_null());
        assert!(player_shot_dispatcher(null()).is_null());
        assert!(player_change_dispatcher(null()).is_null());
        assert!(player_click_dispatcher(null()).is_null());
        assert!(player_check_dispatcher(null()).is_null());
        assert!(player_update_dispatcher(null()).is_null());

        assert!(!add_player_connect_handler(null(), null()));
        assert!(!add_player_spawn_handler(null(), null()));
        assert!(!add_player_text_handler(null(), null()));
        assert!(!add_player_damage_handler(null(), null()));
        assert!(!add_player_update_handler(null(), null()));
    }
}

#[test]
fn vehicles_refuse_a_null_component() {
    unsafe {
        assert!(create_vehicle(null(), 411, ZERO, 0.0, -1, -1, -1, false).is_null());
        assert!(vehicle_by_id(null(), 0).is_null());
        assert!(vehicle_event_dispatcher(null()).is_null());
        assert!(!add_vehicle_handler(null(), null()));

        assert_eq!(vehicle_model(null()), 0);
        assert_eq!(vehicle_health(null()), 0.0);
        assert_eq!(vehicle_id(null()), -1);
        assert_eq!(vehicle_position(null()), ZERO);
        vehicle_set_health(null(), 100.0);
        vehicle_set_colour(null(), 1, 2);
    }
}

#[test]
fn world_entities_refuse_a_null_component() {
    unsafe {
        assert!(create_object(null(), 1337, ZERO, ZERO, 0.0).is_null());
        assert!(create_pickup(null(), 1274, 1, ZERO, 0, true).is_null());
        assert!(create_textdraw(null(), Vector2 { x: 0.0, y: 0.0 }, "text").is_null());
        assert!(create_actor(null(), 46, ZERO, 0.0).is_null());
        assert!(
            create_gangzone(
                null(),
                GangZonePos {
                    min: Vector2 { x: 0.0, y: 0.0 },
                    max: Vector2 { x: 1.0, y: 1.0 },
                },
            )
            .is_null()
        );
        assert!(
            create_textlabel(
                null(),
                "text",
                Colour::rgba(255, 255, 255, 255),
                ZERO,
                10.0,
                0,
                true,
            )
            .is_null()
        );
        assert!(create_menu(null(), "title", Vector2 { x: 0.0, y: 0.0 }, 1, 100.0, 0.0).is_null());

        let weapons = [WeaponSlot::default(); MAX_WEAPON_SLOTS];
        assert!(create_class(null(), 46, 0, ZERO, 0.0, &weapons).is_null());

        assert_eq!(entity_id(null()), -1);
    }
}

#[test]
fn the_extension_map_reports_nothing_for_a_null_extensible() {
    unsafe {
        assert!(crate::omp::extensions::extension(null(), 0xbc03_376a_a359_1a11).is_null());
        assert!(player_extension(null(), 0xbc03_376a_a359_1a11).is_null());
    }
}
