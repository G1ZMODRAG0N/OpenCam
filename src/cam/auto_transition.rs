use std::sync::atomic::Ordering;

use rand::{Rng, RngExt};

use crate::cam::{
    AUTO_ROTATE, AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_CW, AUTO_ROTATE_OFF,
    AUTO_TRANSITION_MAX_DIST, AUTO_TRANSITION_MIN_DIST, AUTO_TRANSITION_NEXT,
    AUTO_TRANSITION_ROT_SPEED, AUTO_TRANSITION_TIME_SECONDS, AUTO_TRANSITION_ZOOM_DIRECTION,
    CAMERA_MODE, PLAYER_INDEX, get_dist_offset_value, now_ms, read_lock_value,
    set_dist_offset_value, write_lock_value,
};

pub fn _auto_transition_mode(player_count: usize, camera_mode: i8) {
    let current_time = now_ms();
    let next_transition_time = AUTO_TRANSITION_NEXT.load(Ordering::Relaxed);
    let transition_time_ms = AUTO_TRANSITION_TIME_SECONDS * 1000;
    let auto_rotate_on = AUTO_ROTATE.load(Ordering::Relaxed);
    let auto_rotate_speed = read_lock_value(&AUTO_ROTATE_CURRENT_SPEED);
    let dist_offset = get_dist_offset_value();
    let dist_direction = read_lock_value(&AUTO_TRANSITION_ZOOM_DIRECTION);

    //force auto rotate
    if !auto_rotate_on {
        AUTO_ROTATE.store(true, Ordering::Relaxed);
    }

    //force cw rotation
    if auto_rotate_speed != AUTO_ROTATE_CW && camera_mode != 2 {
        write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_CW);
    } else if auto_rotate_speed != AUTO_ROTATE_OFF {
        write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_OFF);
    }

    //slow zoom in/out
    if dist_offset > AUTO_TRANSITION_MAX_DIST && dist_direction != -1.0 {
        write_lock_value(&AUTO_TRANSITION_ZOOM_DIRECTION, -1.0);
    }
    if dist_offset < AUTO_TRANSITION_MIN_DIST && dist_direction != 1.0 {
        write_lock_value(&AUTO_TRANSITION_ZOOM_DIRECTION, 1.0);
    }
    set_dist_offset_value(dist_offset + AUTO_TRANSITION_ROT_SPEED * dist_direction);

    //mode transitioning
    if player_count <= 1 {
        if camera_mode != 1 {
            CAMERA_MODE.store(1, Ordering::Relaxed);
        }
    } else {
        let mut next_mode = camera_mode + 1;
        if next_mode > 2 {
            next_mode = 0;
        }

        //trigger on time reached
        if current_time >= next_transition_time {
            // log::info!("next transition");
            //randomize player index everytime we enter the mode
            let count = player_count; // already loaded earlier in the task, reuse?
            if count > 1 {
                let random_index = rand::rng().random_range(1..count) as i32; // skip 0 (main player)
                PLAYER_INDEX.store(random_index, Ordering::Relaxed);
            }
            if next_transition_time != 0 {
                CAMERA_MODE.store(next_mode as i8, Ordering::Relaxed);
            }
            AUTO_TRANSITION_NEXT.store(current_time + transition_time_ms, Ordering::Relaxed);
        }
    }
}
