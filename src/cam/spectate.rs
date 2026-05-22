use crate::cam::{
    get_absolute_return_position, set_dist_offset_value, PLAYER_INDEX, PREVIOUS_CAMERA_MODE,
};
use eldenring::{cs::WorldChrMan, position::HavokPosition};
use fromsoftware_shared::FromStatic;
use std::sync::atomic::Ordering;

pub fn _spectate_mode(player_count: usize) {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
        return;
    };

    let previous_mode = PREVIOUS_CAMERA_MODE.load(Ordering::Relaxed);

    //reset distance offset
    if previous_mode != 0 {
        set_dist_offset_value(5.0);
    }

    //store current mode
    PREVIOUS_CAMERA_MODE.store(0, Ordering::Relaxed);

    //current viewed player
    let target_player_index = PLAYER_INDEX.load(Ordering::Relaxed);
    //set default player position as host's return position
    let mut target_player_pos = Some(get_absolute_return_position());

    //if not host get new target position
    if target_player_index > 0 {
        let mut players = world_chr_man.player_chr_set.characters().enumerate();
        if let Some(player) = players.nth(target_player_index as usize) {
            target_player_pos = Some(player.1.chr_ins.module_container.physics.position);
        }

        //sanity check for default pos
        if player_count < 2 {
            target_player_pos = Some(get_absolute_return_position());
        }

        //set to new target position
        if let Some(ref mut main_player) = world_chr_man.main_player {
            if let Some(player_pos) = target_player_pos {
                let new_position =
                    HavokPosition(player_pos.0, player_pos.1, player_pos.2, player_pos.3);
                main_player.chr_ins.module_container.physics.position = new_position;
            }
        }
    }
}
