use eldenring::cs::WorldChrMan;
use fromsoftware_shared::{F32Vector4, FromStatic};

use crate::cam::{get_dist_offset_value, read_lock_value, CAMERA_COLLISION, FOV, FPS_BOOST};

use std::{mem::transmute, mem::zeroed, sync::atomic::Ordering};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CSEventSimpleInfo {
    vtable: *mut usize,
    unk8: bool,
    unk9: [u8; 0x1f],
    freeze_player: bool,
    unk29: [u8; 0x03],
    freeze_plyr_delay: f32,
    is_transparent: bool,
    unk31: bool,
    unk32: [u8; 0x05],
}

pub fn fade_screen_out(
    fade_to_color_va: u64,
    color: F32Vector4,
    fade_time: f32,
    freeze_player: bool,
) {
    type FadeToColorFn =
        unsafe extern "C" fn(*mut CSEventSimpleInfo, *mut F32Vector4, f32, bool, f32);
    let fade_to_color_fn: FadeToColorFn = unsafe { std::mem::transmute(fade_to_color_va) };
    let fade_info = unsafe {
        &mut CSEventSimpleInfo {
            vtable: std::mem::zeroed(),
            unk8: false,
            unk9: std::mem::zeroed(),
            freeze_player: freeze_player,
            unk29: std::mem::zeroed(),
            freeze_plyr_delay: 0.0,
            is_transparent: true,
            unk31: false,
            unk32: std::mem::zeroed(),
        }
    };
    let fade_color = &mut F32Vector4(color.0, color.1, color.2, 1.0);
    unsafe {
        fade_to_color_fn(
            fade_info,
            fade_color,
            fade_time,
            fade_info.freeze_player,
            fade_info.freeze_plyr_delay,
        )
    };
}

pub fn fade_screen_in(
    fade_to_color_va: u64,
    color: F32Vector4,
    fade_time: f32,
    freeze_player: bool,
) {
    type FadeToColorFn =
        unsafe extern "C" fn(*mut CSEventSimpleInfo, *mut F32Vector4, f32, bool, f32);
    let fade_to_color_fn: FadeToColorFn = unsafe { std::mem::transmute(fade_to_color_va) };
    let fade_info = unsafe {
        &mut CSEventSimpleInfo {
            vtable: std::mem::zeroed(),
            unk8: false,
            unk9: std::mem::zeroed(),
            freeze_player: freeze_player,
            unk29: std::mem::zeroed(),
            freeze_plyr_delay: 0.0,
            is_transparent: true,
            unk31: false,
            unk32: std::mem::zeroed(),
        }
    };
    let fade_color = &mut F32Vector4(color.0, color.1, color.2, 0.0);
    unsafe {
        fade_to_color_fn(
            fade_info,
            fade_color,
            fade_time,
            fade_info.freeze_player,
            fade_info.freeze_plyr_delay,
        )
    };
}

pub fn camera_properties(camera_mode: i8, cam_wall_collision_va: u64) {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
        return;
    };
    //log::info!("cam settings");
    unsafe {
        //get cam ptr
        let world_chr_man_ptr = world_chr_man as *const _ as *const u8;
        let chr_cam_ptr = *(world_chr_man_ptr.add(0x1ece0) as *const *const u8);
        if chr_cam_ptr.is_null() {
            return;
        }

        //get chr follow cam ptr
        let chr_follow_cam_ptr = *(chr_cam_ptr.add(0x60) as *const *const u8);
        if chr_follow_cam_ptr.is_null() {
            return;
        }

        //set distance
        let dist_ptr = chr_follow_cam_ptr.add(0x1b4) as *mut f32;
        *dist_ptr = get_dist_offset_value();

        //set fov
        let fov_ptr = chr_follow_cam_ptr.add(0x50) as *mut f32;
        *fov_ptr = read_lock_value(&FOV);

        //set chase rate aka lerp
        let chase_rate = chr_follow_cam_ptr.add(0x1cc) as *mut f32;
        let chase_rate_dist = chr_follow_cam_ptr.add(0x1fc) as *mut f32;
        let chase_rate_y = chr_follow_cam_ptr.add(0x1f0) as *mut f32;
        let chase_rate_xz = chr_follow_cam_ptr.add(0x1e4) as *mut f32;
        let chase_rate_rotx = chr_follow_cam_ptr.add(0x1e0) as *mut f32;
        let chase_rate_max_angle = chr_follow_cam_ptr.add(0x1f8) as *mut f32;

        //apply to only player focused modes
        match camera_mode {
            //spectate
            0 => {
                *chase_rate = 0.06;
                *chase_rate_dist = 1.0;
                *chase_rate_y = 0.1;
                *chase_rate_xz = 0.1;
                *chase_rate_rotx = 0.3;
                *chase_rate_max_angle = 1.0;
            }
            //top-down
            1 => {
                *chase_rate = 0.06;
                *chase_rate_dist = 1.0;
                *chase_rate_y = 1.0;
                *chase_rate_xz = 0.1;
                *chase_rate_rotx = 0.3;
                *chase_rate_max_angle = 0.0;
            }
            //follow
            2 => {
                *chase_rate = 0.03;
                *chase_rate_dist = 0.01;
                *chase_rate_y = 0.1;
                *chase_rate_xz = 0.1;
                *chase_rate_rotx = 1.0;
                *chase_rate_max_angle = 1.0;
            }
            _ => {}
        }

        //set fps boost if enabled
        let boost_mode_on = FPS_BOOST.load(Ordering::Relaxed);
        let draw_plane_ptr = chr_follow_cam_ptr.add(0x5C) as *mut f32;

        //apply fps boost
        if boost_mode_on {
            *draw_plane_ptr = 100.0;
        } else {
            *draw_plane_ptr = 1000.0;
        }

        //disable camera collisions
        let is_collison_on = CAMERA_COLLISION.load(Ordering::Relaxed);
        let cam_wall_coll_ptr = cam_wall_collision_va as *mut u8;
        let nop_bytes = [0x90; 5];
        let original_bytes_ptr = [0xe8, 0xc1, 0xb8, 0xc0, 0x00];

        if !is_collison_on {
            if *cam_wall_coll_ptr == 0xe8 {
                std::ptr::copy_nonoverlapping(
                    nop_bytes.as_ptr(),
                    cam_wall_coll_ptr,
                    nop_bytes.len(),
                );
            }
        } else {
            if *cam_wall_coll_ptr == 0x90 {
                std::ptr::copy_nonoverlapping(
                    original_bytes_ptr.as_ptr(),
                    cam_wall_coll_ptr,
                    original_bytes_ptr.len(),
                );
            }
        }

        //orbit correction
    }
}
