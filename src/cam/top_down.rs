use crate::cam::{
    CAMERA_MODE, DEFAULT_POS, MARKER_POS_LOCAL, PREVIOUS_CAMERA_MODE, ZEROED_POSITION,
    get_absolute_marker_position, read_lock_value, set_dist_offset_value,
};
use eldenring::cs::{MultiplayRole, WorldChrMan};
use fromsoftware_shared::FromStatic;
use std::sync::atomic::Ordering;

pub fn _top_down_mode() {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return;
    };

    let mut players = world_chr_man.player_chr_set.characters();

    let invaders_present = players.any(|player_ins| unsafe {
        player_ins.player_game_data.as_ref().multiplay_role == MultiplayRole::RedInvasionA
            || player_ins.player_game_data.as_ref().multiplay_role == MultiplayRole::RedInvasionB
    });

    //skip top-down if its an invader lobby
    if invaders_present {
        CAMERA_MODE.store(3, Ordering::Relaxed);
    } else {
        let previous_mode = PREVIOUS_CAMERA_MODE.load(Ordering::Relaxed);
        let custom_marker = get_absolute_marker_position();
        let is_custom_off = custom_marker == ZEROED_POSITION;

        //set defualt distance offset
        if previous_mode != 1 {
            set_dist_offset_value(15.0);
        }

        //set current mode as previous
        PREVIOUS_CAMERA_MODE.store(1, Ordering::Relaxed);

        //set position for top down view
        if let Some(ref mut main_player) = world_chr_man.main_player {
            //apply custom marker if enabled and not zeroed
            if is_custom_off {
                main_player.chr_ins.modules.physics.position = read_lock_value(&DEFAULT_POS);
            } else {
                main_player.chr_ins.modules.physics.position = custom_marker;
            }
        };
    }
}
