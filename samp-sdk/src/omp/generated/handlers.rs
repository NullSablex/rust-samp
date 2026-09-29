//! Event handlers of the groups the SDK does not write by hand — generated
//! by `cargo xtask gen-omp` from the open.mp headers. Do not edit.
//!
//! Each handler's vtable follows its C++ declaration order, which is the
//! slot order on both ABIs: none of these declares a destructor or an
//! overload. `DEFAULT` does what each C++ body does. Narrow integer
//! arguments arrive as a whole word; the value is its low bits.

#![allow(unused_imports)]

use crate::omp::containers::{FlatSet, HybridString, Pair, Span};
use crate::omp::dispatch::event_handler;
use crate::omp::types::{
    Colour, GTAQuat, Hours, Microseconds, Milliseconds, Minutes, Seconds, StringView, TimePoint,
    Vector2, Vector3, Vector4,
};
use crate::omp::*;

event_handler! {
    /// `ActorEventHandler` in `Server/Components/Actors/actors.hpp`.
    ActorHandlerVTable for ActorHandler {
        /// `void onPlayerGiveDamageActor(IPlayer &, IActor &, float, unsigned int, BodyPart)`.
        on_player_give_damage_actor: fn(*mut IPlayer, *mut IActor, f32, u32, i32) = (),
        /// `void onActorStreamOut(IActor &, IPlayer &)`.
        on_actor_stream_out: fn(*mut IActor, *mut IPlayer) = (),
        /// `void onActorStreamIn(IActor &, IPlayer &)`.
        on_actor_stream_in: fn(*mut IActor, *mut IPlayer) = (),
    }
}

event_handler! {
    /// `ClassEventHandler` in `Server/Components/Classes/classes.hpp`.
    ClassHandlerVTable for ClassHandler {
        /// `bool onPlayerRequestClass(IPlayer &, unsigned int)`.
        on_player_request_class: fn(*mut IPlayer, u32) -> bool = true,
    }
}

event_handler! {
    /// `ConsoleEventHandler` in `Server/Components/Console/console.hpp`.
    ConsoleHandlerVTable for ConsoleHandler {
        /// `bool onConsoleText(StringView, StringView, const ConsoleCommandSenderData &)`.
        on_console_text: fn(StringView, StringView, *const ConsoleCommandSenderData) -> bool = false,
        /// `void onRconLoginAttempt(IPlayer &, StringView, bool)`.
        on_rcon_login_attempt: fn(*mut IPlayer, StringView, u32) = (),
        /// `void onConsoleCommandListRequest(FlatHashSet<StringView> &)`.
        on_console_command_list_request: fn(*mut std::ffi::c_void) = (),
    }
}

event_handler! {
    /// `CoreEventHandler` in `core.hpp`.
    CoreHandlerVTable for CoreHandler {
        /// `void onTick(Microseconds, TimePoint)`.
        on_tick: fn(Microseconds, TimePoint),
    }
}

event_handler! {
    /// `GangZoneEventHandler` in `Server/Components/GangZones/gangzones.hpp`.
    GangZoneHandlerVTable for GangZoneHandler {
        /// `void onPlayerEnterGangZone(IPlayer &, IGangZone &)`.
        on_player_enter_gang_zone: fn(*mut IPlayer, *mut IGangZone) = (),
        /// `void onPlayerLeaveGangZone(IPlayer &, IGangZone &)`.
        on_player_leave_gang_zone: fn(*mut IPlayer, *mut IGangZone) = (),
        /// `void onPlayerClickGangZone(IPlayer &, IGangZone &)`.
        on_player_click_gang_zone: fn(*mut IPlayer, *mut IGangZone) = (),
    }
}

event_handler! {
    /// `HTTPResponseHandler` in `core.hpp`.
    HTTPResponseHandlerVTable for HTTPResponseHandler {
        /// `void onHTTPResponse(int, StringView)`.
        on_http_response: fn(i32, StringView),
    }
}

event_handler! {
    /// `MenuEventHandler` in `Server/Components/Menus/menus.hpp`.
    MenuHandlerVTable for MenuHandler {
        /// `void onPlayerSelectedMenuRow(IPlayer &, MenuRow)`.
        on_player_selected_menu_row: fn(*mut IPlayer, u32) = (),
        /// `void onPlayerExitedMenu(IPlayer &)`.
        on_player_exited_menu: fn(*mut IPlayer) = (),
    }
}

event_handler! {
    /// `NPCEventHandler` in `Server/Components/NPCs/npcs.hpp`.
    NPCHandlerVTable for NPCHandler {
        /// `void onNPCFinishMove(INPC &)`.
        on_npc_finish_move: fn(*mut INPC) = (),
        /// `void onNPCCreate(INPC &)`.
        on_npc_create: fn(*mut INPC) = (),
        /// `void onNPCDestroy(INPC &)`.
        on_npc_destroy: fn(*mut INPC) = (),
        /// `void onNPCSpawn(INPC &)`.
        on_npc_spawn: fn(*mut INPC) = (),
        /// `void onNPCRespawn(INPC &)`.
        on_npc_respawn: fn(*mut INPC) = (),
        /// `void onNPCWeaponStateChange(INPC &, PlayerWeaponState, PlayerWeaponState)`.
        on_npc_weapon_state_change: fn(*mut INPC, i32, i32) = (),
        /// `bool onNPCTakeDamage(INPC &, IPlayer &, float, uint8_t, BodyPart)`.
        on_npc_take_damage: fn(*mut INPC, *mut IPlayer, f32, u32, i32) -> bool = true,
        /// `bool onNPCGiveDamage(INPC &, IPlayer &, float, uint8_t, BodyPart)`.
        on_npc_give_damage: fn(*mut INPC, *mut IPlayer, f32, u32, i32) -> bool = true,
        /// `void onNPCDeath(INPC &, IPlayer *, int)`.
        on_npc_death: fn(*mut INPC, *mut IPlayer, i32) = (),
        /// `bool onNPCShotMissed(INPC &, const PlayerBulletData &)`.
        on_npc_shot_missed: fn(*mut INPC, *const PlayerBulletData) -> bool = true,
        /// `bool onNPCShotPlayer(INPC &, IPlayer &, const PlayerBulletData &)`.
        on_npc_shot_player: fn(*mut INPC, *mut IPlayer, *const PlayerBulletData) -> bool = true,
        /// `bool onNPCShotNPC(INPC &, INPC &, const PlayerBulletData &)`.
        on_npc_shot_npc: fn(*mut INPC, *mut INPC, *const PlayerBulletData) -> bool = true,
        /// `bool onNPCShotVehicle(INPC &, IVehicle &, const PlayerBulletData &)`.
        on_npc_shot_vehicle: fn(*mut INPC, *mut IVehicle, *const PlayerBulletData) -> bool = true,
        /// `bool onNPCShotObject(INPC &, IObject &, const PlayerBulletData &)`.
        on_npc_shot_object: fn(*mut INPC, *mut IObject, *const PlayerBulletData) -> bool = true,
        /// `bool onNPCShotPlayerObject(INPC &, IPlayerObject &, const PlayerBulletData &)`.
        on_npc_shot_player_object: fn(*mut INPC, *mut IPlayerObject, *const PlayerBulletData) -> bool = true,
        /// `void onNPCPlaybackStart(INPC &, int)`.
        on_npc_playback_start: fn(*mut INPC, i32) = (),
        /// `void onNPCPlaybackEnd(INPC &, int)`.
        on_npc_playback_end: fn(*mut INPC, i32) = (),
        /// `void onNPCFinishNodePoint(INPC &, int, uint16_t)`.
        on_npc_finish_node_point: fn(*mut INPC, i32, u32) = (),
        /// `void onNPCFinishNode(INPC &, int)`.
        on_npc_finish_node: fn(*mut INPC, i32) = (),
        /// `bool onNPCChangeNode(INPC &, int, int)`.
        on_npc_change_node: fn(*mut INPC, i32, i32) -> bool = true,
        /// `void onNPCFinishMovePathPoint(INPC &, int, int)`.
        on_npc_finish_move_path_point: fn(*mut INPC, i32, i32) = (),
        /// `void onNPCFinishMovePath(INPC &, int)`.
        on_npc_finish_move_path: fn(*mut INPC, i32) = (),
    }
}

event_handler! {
    /// `ObjectEventHandler` in `Server/Components/Objects/objects.hpp`.
    ObjectHandlerVTable for ObjectHandler {
        /// `void onMoved(IObject &)`.
        on_moved: fn(*mut IObject) = (),
        /// `void onPlayerObjectMoved(IPlayer &, IPlayerObject &)`.
        on_player_object_moved: fn(*mut IPlayer, *mut IPlayerObject) = (),
        /// `void onObjectSelected(IPlayer &, IObject &, int, Vector3)`.
        on_object_selected: fn(*mut IPlayer, *mut IObject, i32, Vector3) = (),
        /// `void onPlayerObjectSelected(IPlayer &, IPlayerObject &, int, Vector3)`.
        on_player_object_selected: fn(*mut IPlayer, *mut IPlayerObject, i32, Vector3) = (),
        /// `void onObjectEdited(IPlayer &, IObject &, ObjectEditResponse, Vector3, Vector3)`.
        on_object_edited: fn(*mut IPlayer, *mut IObject, i32, Vector3, Vector3) = (),
        /// `void onPlayerObjectEdited(IPlayer &, IPlayerObject &, ObjectEditResponse, Vector3, Vector3)`.
        on_player_object_edited: fn(*mut IPlayer, *mut IPlayerObject, i32, Vector3, Vector3) = (),
        /// `void onPlayerAttachedObjectEdited(IPlayer &, int, bool, const ObjectAttachmentSlotData &)`.
        on_player_attached_object_edited: fn(*mut IPlayer, i32, u32, *const ObjectAttachmentSlotData) = (),
    }
}

event_handler! {
    /// `OptionEnumeratorCallback` in `core.hpp`.
    OptionEnumeratorCallbackVTable for OptionEnumeratorCallback {
        /// `bool proc(StringView, ConfigOptionType)`.
        proc: fn(StringView, i32) -> bool,
    }
}

event_handler! {
    /// `PickupEventHandler` in `Server/Components/Pickups/pickups.hpp`.
    PickupHandlerVTable for PickupHandler {
        /// `void onPlayerPickUpPickup(IPlayer &, IPickup &)`.
        on_player_pick_up_pickup: fn(*mut IPlayer, *mut IPickup) = (),
    }
}

event_handler! {
    /// `PlayerCheckpointEventHandler` in `Server/Components/Checkpoints/checkpoints.hpp`.
    PlayerCheckpointHandlerVTable for PlayerCheckpointHandler {
        /// `void onPlayerEnterCheckpoint(IPlayer &)`.
        on_player_enter_checkpoint: fn(*mut IPlayer) = (),
        /// `void onPlayerLeaveCheckpoint(IPlayer &)`.
        on_player_leave_checkpoint: fn(*mut IPlayer) = (),
        /// `void onPlayerEnterRaceCheckpoint(IPlayer &)`.
        on_player_enter_race_checkpoint: fn(*mut IPlayer) = (),
        /// `void onPlayerLeaveRaceCheckpoint(IPlayer &)`.
        on_player_leave_race_checkpoint: fn(*mut IPlayer) = (),
    }
}

event_handler! {
    /// `PlayerDialogEventHandler` in `Server/Components/Dialogs/dialogs.hpp`.
    PlayerDialogHandlerVTable for PlayerDialogHandler {
        /// `void onDialogResponse(IPlayer &, int, DialogResponse, int, StringView)`.
        on_dialog_response: fn(*mut IPlayer, i32, i32, i32, StringView) = (),
    }
}

event_handler! {
    /// `PlayerModelsEventHandler` in `Server/Components/CustomModels/custommodels.hpp`.
    PlayerModelsHandlerVTable for PlayerModelsHandler {
        /// `void onPlayerFinishedDownloading(IPlayer &)`.
        on_player_finished_downloading: fn(*mut IPlayer) = (),
        /// `bool onPlayerRequestDownload(IPlayer &, ModelDownloadType, uint32_t)`.
        on_player_request_download: fn(*mut IPlayer, u32, u32) -> bool = true,
    }
}

event_handler! {
    /// `PoolEventHandler<IPlayer>` in `pool.hpp`.
    PlayerPoolHandlerVTable for PlayerPoolHandler {
        /// `void onPoolEntryCreated(IPlayer &)`.
        on_pool_entry_created: fn(*mut IPlayer) = (),
        /// `void onPoolEntryDestroyed(IPlayer &)`.
        on_pool_entry_destroyed: fn(*mut IPlayer) = (),
    }
}

event_handler! {
    /// `TextDrawEventHandler` in `Server/Components/TextDraws/textdraws.hpp`.
    TextDrawHandlerVTable for TextDrawHandler {
        /// `void onPlayerClickTextDraw(IPlayer &, ITextDraw &)`.
        on_player_click_text_draw: fn(*mut IPlayer, *mut ITextDraw) = (),
        /// `void onPlayerClickPlayerTextDraw(IPlayer &, IPlayerTextDraw &)`.
        on_player_click_player_text_draw: fn(*mut IPlayer, *mut IPlayerTextDraw) = (),
        /// `bool onPlayerCancelTextDrawSelection(IPlayer &)`.
        on_player_cancel_text_draw_selection: fn(*mut IPlayer) -> bool = false,
        /// `bool onPlayerCancelPlayerTextDrawSelection(IPlayer &)`.
        on_player_cancel_player_text_draw_selection: fn(*mut IPlayer) -> bool = false,
    }
}
