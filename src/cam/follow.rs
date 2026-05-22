use crate::cam::{
    set_dist_offset_value, write_lock_value, AUTO_ROTATE, AUTO_ROTATE_CURRENT_SPEED,
    AUTO_ROTATE_OFF, CAMERA_MODE, DIST_MIN, FOV, ORBIT_CORRECTION, ORBIT_CORRECTION_THRESHOLD,
    PREVIOUS_CAMERA_MODE,
};
use eldenring::{
    cs::{PlayerIns, WorldChrMan},
    position::HavokPosition,
};
use fromsoftware_shared::{F32Vector4, FromStatic};
use std::sync::atomic::Ordering;

pub fn _follow_mode(player_count: usize, field_area_va: u64, fade_to_color_va: u64) {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
        return;
    };

    //set to mode 0 if there are not enough players. need 3(including host) or more to derive a center point
    if player_count < 3 {
        CAMERA_MODE.store(0, Ordering::Relaxed);
    } else {
        PREVIOUS_CAMERA_MODE.store(2, Ordering::Relaxed);
        let player_sets: Vec<&mut PlayerIns> = world_chr_man.player_chr_set.characters().collect();

        //remove any auto rotation
        let auto_rotate_on = AUTO_ROTATE.load(Ordering::Relaxed);
        if auto_rotate_on {
            AUTO_ROTATE.store(false, Ordering::Relaxed);
            write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_OFF);
        }

        let mut cam_coords = F32Vector4(0.0, 0.0, 0.0, 0.0);

        //camera positioning
        unsafe {
            let field_area_instr = field_area_va as *const u8;
            let rip_offset = std::ptr::read_unaligned(field_area_instr.add(3) as *const i32);
            let next_instr = field_area_instr.add(7);
            let global_field_area_ptr = next_instr.offset(rip_offset as isize) as *const *mut u8;

            let field_area_base = *global_field_area_ptr;
            if field_area_base.is_null() {
                return;
            }

            let ptr_20 = *(field_area_base.add(0x20) as *const *mut u8);
            if ptr_20.is_null() {
                return;
            }

            let cs_cam = *(ptr_20.add(0x18) as *const *mut u8);
            if cs_cam.is_null() {
                return;
            }

            let view_matrix_ptr = cs_cam.add(0x40) as *mut F32Vector4;

            cam_coords = *view_matrix_ptr;
        }

        // log::info!(
        //     "Cam X: {}, Y: {}, Z: {}",
        //     cam_coords.0,
        //     cam_coords.1,
        //     cam_coords.2
        // );
        let mut pos_x_total: Vec<f32> = vec![];
        let mut pos_y_total: Vec<f32> = vec![];
        let mut pos_z_total: Vec<f32> = vec![];
        let mut pos_w_total: Vec<f32> = vec![];

        //orbit correction

        let mut player_too_close = false;
        //get positional data for all players aside main
        for player_ins in player_sets {
            let is_main_player = player_ins.player_game_data.is_main_player;
            if is_main_player {
                continue;
            }
            let pos = player_ins.chr_ins.module_container.physics.position;
            pos_x_total.push(pos.0);
            pos_y_total.push(pos.1);
            pos_z_total.push(pos.2);
            pos_w_total.push(pos.3);

            let distance_x = cam_coords.0 - pos.0;
            let distance_y = cam_coords.1 - pos.1;
            let distance_z = cam_coords.2 - pos.2;

            let distance_sq =
                (distance_x * distance_x) + (distance_y * distance_y) + (distance_z * distance_z);
            // log::info!("distance_sq: {}", distance_sq);

            if distance_sq < ORBIT_CORRECTION_THRESHOLD {
                player_too_close = true;
            };
            // log::info!("orbit correction");
        }

        let orbit_on = ORBIT_CORRECTION.load(Ordering::Relaxed);
        if player_too_close {
            if !orbit_on {
                ORBIT_CORRECTION.store(true, Ordering::Relaxed);
            }
        } else {
            if orbit_on {
                ORBIT_CORRECTION.store(false, Ordering::Relaxed);
            }
        }

        //distance correction
        //reinforce client count
        let total_clients = player_count - 1;

        //get average position based on total player count
        //pos1+pos2/p_count
        let follow_pos = HavokPosition(
            pos_x_total.iter().sum::<f32>() / (total_clients as f32),
            pos_y_total.iter().sum::<f32>() / (total_clients as f32),
            pos_z_total.iter().sum::<f32>() / (total_clients as f32),
            0.0,
        );

        //set pos
        if let Some(ref mut main_player) = world_chr_man.main_player {
            main_player.chr_ins.module_container.physics.position = follow_pos;
        }

        //calculate square differences of positions for max dist
        let mut max_sq_dist: f32 = 0.0;
        for i in 0..total_clients as usize {
            let dx = pos_x_total[i] - follow_pos.0;
            let dy = pos_y_total[i] - follow_pos.1;
            let dz = pos_z_total[i] - follow_pos.2;

            let sq_dist = (dx * dx) + (dy * dy) + (dz * dz);
            if sq_dist > max_sq_dist {
                // log::info!("zoom_correction");
                max_sq_dist = sq_dist;
            }
        }

        //calculate and set current target distance based off min and max
        let max_dist_from_center = max_sq_dist.sqrt();
        let fov_padding = 1.25;
        let target_dist = DIST_MIN + max_dist_from_center * fov_padding;

        write_lock_value(&FOV, 1.0);
        set_dist_offset_value(target_dist);
    }
}
