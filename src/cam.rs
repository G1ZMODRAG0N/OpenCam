use std::{
    sync::{
        RwLock,
        atomic::{AtomicBool, AtomicI8, AtomicI32, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eldenring::{
    cs::{
        CSCamExt, CSCamera, CSFeManHudState, CSFeManImp, CSSessionManager, CSTaskGroupIndex,
        CSTaskImp, ChrType, LobbyState, MenuString, MultiplayRole, PlayerIns, WorldBlockInfo,
        WorldChrMan,
    },
    fd4::FD4TaskData,
    position::HavokPosition,
    util::system::wait_for_system_init,
};

use fromsoftware_shared::{F32Vector4, FromStatic, Program, SharedTaskImpExt};
use pelite::{pattern, pe::Pe};
use retour::static_detour;

pub(crate) mod auto_transition;
pub(crate) mod cam_properties;
pub(crate) mod follow;
pub(crate) mod lock_on_hud;
pub(crate) mod spectate;
pub(crate) mod top_down;

use crate::{
    InitError,
    cam::{
        auto_transition::_auto_transition_mode,
        cam_properties::{camera_properties, fade_screen_in, fade_screen_out},
        follow::_follow_mode,
        lock_on_hud::lock_on_hud_props,
        spectate::_spectate_mode,
        top_down::_top_down_mode,
    },
    rva,
};

pub const TARGET_MAN_PTRN: &[pelite::pattern::Atom] =
    pattern!("48 8B 05 ? ? ? ? 0F B6 5B ? 48 85 C0");

//inputs
pub static CURRENT_INPUT: AtomicI32 = AtomicI32::new(0);
pub static LAST_INPUT_TIME_MS: AtomicU64 = AtomicU64::new(0);
pub const DEBOUNCE_MS: u64 = 170;
pub static CAMERA_BYPASS: AtomicBool = AtomicBool::new(false);

//window
pub static WINDOW_INFOCUS: AtomicBool = AtomicBool::new(true);

//fade
pub static FADE_DURATION_END: AtomicU64 = AtomicU64::new(0);
pub static FADE_STATE: AtomicI8 = AtomicI8::new(0);
pub const FADE_DURATION_MS: u64 = 2000;

//camera
pub const CAMERA_ITEM_ID: u32 = 4600900;
pub const CAMERA_MARKER_ITEM_ID: u32 = 4601000;
pub static CAMERA_ACTIVATION_ANIM: AtomicBool = AtomicBool::new(false);
pub static CAMERA_ACTIVATION_TRIGGER: AtomicBool = AtomicBool::new(false);
pub static CAMERA_STATE: AtomicI8 = AtomicI8::new(0);
pub static CAMERA_MODE: AtomicI8 = AtomicI8::new(0);
pub static PREVIOUS_CAMERA_MODE: AtomicI8 = AtomicI8::new(0);
pub static CAMERA_MARKER: AtomicBool = AtomicBool::new(false);
pub static FPS_BOOST: AtomicBool = AtomicBool::new(false);

//auto transition
pub static AUTO_TRANSITION: AtomicBool = AtomicBool::new(false);
pub static AUTO_TRANSITION_NEXT: AtomicU64 = AtomicU64::new(0);
pub static AUTO_TRANSITION_MAX_DIST: f32 = 32.0;
pub static AUTO_TRANSITION_MIN_DIST: f32 = 15.0;
pub static AUTO_TRANSITION_ZOOM_DIRECTION: RwLock<f32> = RwLock::new(1.0);
pub static AUTO_TRANSITION_ROT_SPEED: f32 = 0.005;
pub const AUTO_TRANSITION_TIME_SECONDS: u64 = 12;

//players
pub static PLAYER_INDEX: AtomicI32 = AtomicI32::new(0);
pub static PLAYER_COUNT: AtomicUsize = AtomicUsize::new(0);

//position
pub static DEFAULT_POS: RwLock<HavokPosition> = RwLock::new(ZEROED_POSITION);
pub static MARKER_POS_LOCAL: RwLock<HavokPosition> = RwLock::new(ZEROED_POSITION);
pub static RETURN_POS_LOCAL: RwLock<HavokPosition> = RwLock::new(ZEROED_POSITION);
pub static ZEROED_POSITION: HavokPosition = HavokPosition(0.0, 0.0, 0.0, 0.0);
// pub static ZEROED_ORIENTATION: Quaternion = Quaternion(0.0, 0.0, 0.0, 0.0);

//fov
pub static FOV: RwLock<f32> = RwLock::new(1.2);
pub static FOV_MAX: f32 = 1.5;
pub static FOV_MIN: f32 = 0.5;

//zoom
pub static DIST_OFFSET: RwLock<f32> = RwLock::new(0.0);
pub static DIST_MAX: f32 = 25.0;
pub static DIST_MIN: f32 = 3.7;

//orbiting
pub static TOP_DOWN_ORBIT_X_SPEED: f32 = 0.3;
pub static SPECTATE_ORBIT_X_SPEED: f32 = 0.25;
pub static ORBIT_Y_SPEED: f32 = 0.25;
pub static ORBIT_CORRECTION: AtomicBool = AtomicBool::new(false);
pub static ORBIT_CORRECTION_THRESHOLD: f32 = 20.0;
pub static ORBIT_CORRECTION_SPEED: f32 = 0.02;

//auto rotation
pub static AUTO_ROTATE: AtomicBool = AtomicBool::new(false);
pub static AUTO_ROTATE_CURRENT_SPEED: RwLock<f32> = RwLock::new(0.0);
pub static AUTO_ROTATE_OFF: f32 = 0.0;
pub static AUTO_ROTATE_CW: f32 = 0.008;
pub static AUTO_ROTATE_CCW: f32 = -0.008;

//target lock
pub static TARGET_LOCK_MODE: AtomicBool = AtomicBool::new(false);

//collison
pub static CAMERA_COLLISION: AtomicBool = AtomicBool::new(false);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum LabelReplacementTemplate {
    JoinName = 1,
    HostName = 2,
    DeadName = 3,
    LeaveName = 4,
    RoleName = 5,
    Target = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum CameraState {
    Off = 0,
    On = 1,
    Deactivating = 2,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn read_lock_value<T: Copy>(lock: &RwLock<T>) -> T {
    *lock.read().unwrap()
}

fn write_lock_value<T>(lock: &RwLock<T>, value: T) {
    *lock.write().unwrap() = value;
}

pub fn get_absolute_return_position() -> HavokPosition {
    let local = read_lock_value(&RETURN_POS_LOCAL); //*RETURN_POS_LOCAL.read().unwrap();
    let current_center = read_lock_value(&DEFAULT_POS);
    HavokPosition(
        current_center.0 + local.0,
        current_center.1 + local.1,
        current_center.2 + local.2,
        local.3,
    )
}

pub fn get_absolute_marker_position() -> HavokPosition {
    let local = read_lock_value(&MARKER_POS_LOCAL);
    let current_center = read_lock_value(&DEFAULT_POS);
    HavokPosition(
        current_center.0 + local.0,
        current_center.1 + local.1,
        current_center.2 + local.2,
        local.3,
    )
}

pub fn save_return_position_relative(player_pos: HavokPosition) {
    let center = read_lock_value(&DEFAULT_POS);
    let new_pos = HavokPosition(
        player_pos.0 - center.0,
        player_pos.1 - center.1,
        player_pos.2 - center.2,
        player_pos.3,
    );
    write_lock_value(&RETURN_POS_LOCAL, new_pos);
}

pub fn save_marker_position_relative(player_pos: HavokPosition) {
    let center = read_lock_value(&DEFAULT_POS);
    let new_pos = HavokPosition(
        player_pos.0 - center.0,
        player_pos.1 - center.1,
        player_pos.2 - center.2,
        player_pos.3,
    );
    write_lock_value(&MARKER_POS_LOCAL, new_pos);
}

fn get_dist_offset_value() -> f32 {
    read_lock_value(&DIST_OFFSET)
}

fn set_dist_offset_value(new_offset: f32) {
    write_lock_value(&DIST_OFFSET, new_offset);
}

pub fn start_dist_fade_logic(fade_to_color_va: u64) {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
        return;
    };
    if let Ok(cs_cam) = unsafe { CSCamera::instance() } {
        if let Some(ref main_player) = world_chr_man.main_player {
            let player_pos = main_player.chr_ins.modules.physics.position;
            let cam_pos = cs_cam.pers_cam_1.position();

            let distance_x = cam_pos.0 - player_pos.0;
            let distance_z = cam_pos.2 - player_pos.2;

            let distance = (distance_x * distance_x) + (distance_z * distance_z);
            let distance_sqrt = distance.sqrt();
            let fade_state = FADE_STATE.load(Ordering::Relaxed);
            if fade_state == 0 && distance_sqrt > 300.0 {
                let end_time = now_ms() + FADE_DURATION_MS;
                FADE_DURATION_END.store(end_time, Ordering::Relaxed);
                FADE_STATE.store(1, Ordering::Relaxed); //store is faded out
                fade_screen_out(fade_to_color_va, F32Vector4(0.0, 0.0, 0.0, 1.0), 0.3, false);
            }
            if fade_state == 1 && distance_sqrt < 80.0 {
                fade_screen_in(fade_to_color_va, F32Vector4(0.0, 0.0, 0.0, 0.0), 1.0, false);
                FADE_STATE.store(2, Ordering::Relaxed);
            }
            if fade_state == 2 && distance_sqrt < 300.0 {
                FADE_STATE.store(0, Ordering::Relaxed);
            }
        }
    }
}

pub fn start_camera_deactivation(
    cam_wall_collision_va: u64,
    lock_tgt_man: *mut u8,
    is_rotation_locked: *mut u8,
    is_targeting_enabled: *mut u8,
) {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return;
    };

    let Some(ref mut main_player) = world_chr_man.main_player else {
        return;
    };
    unsafe {
        //load state

        let return_pos = get_absolute_return_position();
        main_player.chr_ins.modules.physics.position = return_pos;
        main_player.chr_ins.debug_flags.set_force_unloaded(false);
        main_player.chr_ins.chr_type = ChrType::Local;
        main_player
            .chr_ins
            .modules
            .action_request
            .disabled_action_inputs
            .set_use_item(false);

        //hud restore
        if let Ok(fe_man) = CSFeManImp::instance_mut() {
            //log::info!("hud restore");
            if fe_man.hud_state == CSFeManHudState::HideAll {
                fe_man.hud_state = CSFeManHudState::Default;
            }
        }

        //restore target lock settings
        //log::info!("restore tgt lock");
        if !lock_tgt_man.is_null() {
            *is_rotation_locked = 1;
            *is_targeting_enabled = 1;
            //*is_manual_lock_enabled = 0;
        }
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

        //disable camera collisions
        let cam_wall_coll_ptr = cam_wall_collision_va as *mut u8;
        let original_bytes_ptr = [0xe8, 0xc1, 0xb8, 0xc0, 0x00];

        if *cam_wall_coll_ptr == 0x90 {
            std::ptr::copy_nonoverlapping(
                original_bytes_ptr.as_ptr(),
                cam_wall_coll_ptr,
                original_bytes_ptr.len(),
            );
        }

        //set chase rate aka lerp
        let chase_rate = chr_follow_cam_ptr.add(0x1cc) as *mut f32;
        let chase_rate_dist = chr_follow_cam_ptr.add(0x1fc) as *mut f32;
        let chase_rate_y = chr_follow_cam_ptr.add(0x1f0) as *mut f32;
        let chase_rate_xz = chr_follow_cam_ptr.add(0x1e4) as *mut f32;
        let chase_rate_rotx = chr_follow_cam_ptr.add(0x1e0) as *mut f32;
        let chase_rate_max_angle = chr_follow_cam_ptr.add(0x1f8) as *mut f32;
        //
        *chase_rate = 0.1;
        *chase_rate_dist = 1.0;
        *chase_rate_y = 1.0;
        *chase_rate_xz = 0.1;
        *chase_rate_rotx = 0.3;
        *chase_rate_max_angle = 0.0;

        //reset fps boost
        FPS_BOOST.store(false, Ordering::Relaxed);
        let draw_plane_ptr = chr_follow_cam_ptr.add(0x5C) as *mut f32;
        //apply fps boost
        if *draw_plane_ptr == 100.0 {
            *draw_plane_ptr = 1000.0;
        }

        //states
        PLAYER_INDEX.store(1, Ordering::Relaxed);
        CAMERA_STATE.store(CameraState::Off as i8, Ordering::Relaxed);
        CAMERA_MODE.store(0, Ordering::Relaxed);
    }
}

//fn for displaying net msgs
pub fn display_net_message(notification_va: u64, string: &str) {
    unsafe {
        type ShowKillNotice = unsafe extern "C" fn(
            u16,
            MultiplayRole,
            LabelReplacementTemplate,
            *mut MenuString,
            // i32,
        );

        let show_kill_count_fn: ShowKillNotice = std::mem::transmute(notification_va);
        let net_msg_param = 405u16;

        let name_string: Vec<u16> = format!("{}\0", string).encode_utf16().collect();
        let mut menu_string_mem = [0u8; 0x38];
        let static_string_ptr = menu_string_mem.as_mut_ptr() as *mut *const u16;
        *static_string_ptr = name_string.as_ptr();
        let menu_string_ptr = menu_string_mem.as_mut_ptr() as *mut MenuString;

        show_kill_count_fn(
            net_msg_param,
            MultiplayRole::BattleRoyale,
            LabelReplacementTemplate::DeadName,
            menu_string_ptr,
            // 1,
        );
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum InputId {
    DPadUp = 16,
    DPadDown = 19,
    DPadLeft = 18,
    DPadRight = 17,
    PadL1 = 9,
    PadR1 = 7,
    PadR3 = 20,
    PadL3 = 21,
    PadR2 = 8,
    PadL2 = 10,
    PadTri = 11,
    PadSq = 15,
    PadCir = 12,
    PadX = 14,
    None = 0,
}

impl InputId {
    pub fn check_id(id: i32) -> Option<Self> {
        match id {
            16 => Some(Self::DPadUp),
            19 => Some(Self::DPadDown),
            18 => Some(Self::DPadLeft),
            17 => Some(Self::DPadRight),
            9 => Some(Self::PadL1),
            7 => Some(Self::PadR1),
            20 => Some(Self::PadR3),
            21 => Some(Self::PadL3),
            8 => Some(Self::PadR2),
            10 => Some(Self::PadL2),
            11 => Some(Self::PadTri),
            15 => Some(Self::PadSq),
            12 => Some(Self::PadCir),
            14 => Some(Self::PadX),
            _ => None,
        }
    }
}

static_detour! {
    static POLL_INPUT: unsafe extern "C" fn(*mut u8, i32) -> bool;
    static APPLY_ZOOM_LERP: unsafe extern "C" fn(*mut usize, usize, usize);
    static APPLY_CONTROL_MOVEMENT: unsafe extern "C" fn(*mut u8, f32, u8);
    static STEP_MAP_INIT: unsafe extern "C" fn(*mut usize);
    static TOP_TITLE_DIALOG: unsafe extern "C" fn(
        *mut usize,
        *mut usize,
        *mut usize,
        usize
    ) -> *mut usize;
    static GET_BLOCK_COORDINATES: unsafe extern "C" fn(*mut WorldBlockInfo) -> *mut F32Vector4;
    static ITEM_USED:unsafe extern "C" fn(*mut PlayerIns, *mut u32);
    static CAM_X_SPEED: unsafe extern "C" fn(*mut usize) -> f32;
    static CAM_Y_SPEED: unsafe extern "C" fn(*mut usize) -> f32;
    static GET_ACTION_BTTN_PARAM: unsafe extern "C" fn(*mut usize, i32);
    static WNDPROC: unsafe extern "system" fn(usize, u32, usize, isize) -> isize;
    static GET_RESPAWN_COORDS: unsafe extern "C" fn(*mut usize, *mut F32Vector4, *mut F32Vector4, *mut usize, *mut F32Vector4, *mut PlayerIns);
}

macro_rules! install_detour {
    ($detour_static:ident, $address:expr, $closure:expr) => {
        unsafe {
            $detour_static
                .initialize(std::mem::transmute($address), $closure)
                .map_err(InitError::Retour)?
                .enable()?;
        }
    };
}

pub fn hook(program: &Program) -> Result<(), InitError> {
    wait_for_system_init(program, Duration::MAX).map_err(InitError::CsTaskImp)?;

    //scan for tgt lock man
    let mut scanner = program.scanner().matches_code(TARGET_MAN_PTRN);
    let mut matches = [0; 1];
    if !scanner.next(&mut matches) {
        return Err(InitError::FlakyPattern("TARGET_MAN_PTRN"));
    }
    //assign first match
    let target_lock_man = matches[0];

    let poll_input_va = program
        .rva_to_va(rva::get().poll_input)
        .map_err(InitError::AddressConversion)?;

    let apply_zoom_lerp_va = program
        .rva_to_va(rva::get().apply_zoom_lerp)
        .map_err(InitError::AddressConversion)?;

    let apply_control_movement_va = program
        .rva_to_va(rva::get().apply_control_movement)
        .map_err(InitError::AddressConversion)?;

    let target_lock_man_va = program
        .rva_to_va(target_lock_man)
        .map_err(InitError::AddressConversion)? as *const u8;

    let show_notice_wkill_va = program
        .rva_to_va(rva::get().show_net_notice_wkill)
        .map_err(InitError::AddressConversion)?;

    let show_net_notice_va = program
        .rva_to_va(rva::get().show_net_notice)
        .map_err(InitError::AddressConversion)?;

    let stage_rendered_va = program
        .rva_to_va(rva::get().stage_rendered)
        .map_err(InitError::AddressConversion)?;

    let title_top_dialog_va = program
        .rva_to_va(rva::get().title_top_dialog)
        .map_err(InitError::AddressConversion)?;

    let get_block_coords_va = program
        .rva_to_va(rva::get().get_block_coords)
        .map_err(InitError::AddressConversion)?;

    let item_used_va = program
        .rva_to_va(rva::get().item_used)
        .map_err(InitError::AddressConversion)?;

    let cam_x_speed_va = program
        .rva_to_va(rva::get().cam_x_speed)
        .map_err(InitError::AddressConversion)?;

    let cam_y_speed_va = program
        .rva_to_va(rva::get().cam_y_speed)
        .map_err(InitError::AddressConversion)?;

    let get_action_button_param_va = program
        .rva_to_va(rva::get().get_action_button_param)
        .map_err(InitError::AddressConversion)?;

    let wnd_proc_va = program
        .rva_to_va(rva::get().wnd_proc)
        .map_err(InitError::AddressConversion)?;

    let cam_wall_collision_va = program
        .rva_to_va(rva::get().cam_wall_collision)
        .map_err(InitError::AddressConversion)?;

    let field_area_va = program
        .rva_to_va(rva::get().field_area)
        .map_err(InitError::AddressConversion)?;

    let fade_to_color_va = program
        .rva_to_va(rva::get().fade_to_color)
        .map_err(InitError::AddressConversion)?;

    let step_map_init_va = program
        .rva_to_va(rva::get().step_map_init)
        .map_err(InitError::AddressConversion)?;

    //capture kbm inputs
    install_detour!(
        WNDPROC,
        wnd_proc_va,
        move |h_wnd, u_msg, h_drop, l_param| {
            let result = WNDPROC.call(h_wnd, u_msg, h_drop, l_param);

            let Ok(world_chr_man) = WorldChrMan::instance() else {
                return result;
            };

            let Some(ref main_player) = world_chr_man.main_player else {
                return result;
            };

            let is_host =
                main_player.player_game_data.as_ref().multiplay_role == MultiplayRole::Host;
            let is_overworld = main_player.current_block_id.area() >= 60;
            let player_pos = main_player.chr_ins.modules.physics.position;
            let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
            let camera_on_trigger = CAMERA_ACTIVATION_TRIGGER.load(Ordering::Relaxed);
            let custom_marker_on = CAMERA_MARKER.load(Ordering::Relaxed);

            if u_msg == 0x0007 {
                WINDOW_INFOCUS.store(true, Ordering::Relaxed);
            }

            if u_msg == 0x0008 {
                WINDOW_INFOCUS.store(false, Ordering::Relaxed);
            }

            let window_infocus = WINDOW_INFOCUS.load(Ordering::Relaxed);
            if !window_infocus {
                return result;
            }

            // WM_KEYDOWN = 0x100, WM_KEYUP = 0x101 F-ROW 0x70 -> 0x11 https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-keydown
            if u_msg == 0x100 && is_host {
                match h_drop {
                    0x74 => {
                        if is_overworld && !custom_marker_on {
                            save_marker_position_relative(player_pos);
                            CAMERA_MARKER.store(true, Ordering::Relaxed);
                        }
                        if !camera_on_trigger && camera_state == CameraState::Off as i8 {
                            CAMERA_ACTIVATION_TRIGGER.store(true, Ordering::Relaxed);
                        }
                        if camera_state == CameraState::On as i8 {
                            CAMERA_STATE.store(CameraState::Deactivating as i8, Ordering::Relaxed);
                        }
                    }
                    0x75 => {
                        if !camera_on_trigger && camera_state == CameraState::Off as i8 {
                            if !custom_marker_on {
                                save_marker_position_relative(player_pos);
                                display_net_message(
                                    show_net_notice_va,
                                    "New camera center position has been set",
                                );
                                CAMERA_MARKER.store(true, Ordering::Relaxed);
                            } else if custom_marker_on && !is_overworld {
                                write_lock_value(&MARKER_POS_LOCAL, ZEROED_POSITION);
                                display_net_message(
                                    show_net_notice_va,
                                    "Camera center position has been reset",
                                );
                                CAMERA_MARKER.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    0x76 => {}
                    0x77 => {
                        let bypass_on = CAMERA_BYPASS.load(Ordering::Relaxed);
                        if bypass_on {
                            CAMERA_BYPASS.store(false, Ordering::Relaxed);
                            display_net_message(show_net_notice_va, "Bypass Disabled");
                        } else {
                            CAMERA_BYPASS.store(true, Ordering::Relaxed);
                            display_net_message(show_net_notice_va, "Bypass Enabled");
                        }
                    }
                    _ => {}
                }
            }

            return result;
        }
    );

    //remove action button prompts from displaying while in cam view
    install_detour!(
        GET_ACTION_BTTN_PARAM,
        get_action_button_param_va,
        |param_1, param_2| {
            let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
            if camera_state == CameraState::On as i8 {
                return;
            } else {
                GET_ACTION_BTTN_PARAM.call(param_1, param_2);
            }
        }
    );

    //apply max speed to Y orbit
    install_detour!(CAM_Y_SPEED, cam_y_speed_va, |in_game_pad| {
        let mut result = CAM_Y_SPEED.call(in_game_pad);
        let max_speed = ORBIT_Y_SPEED;
        let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
        let camera_mode = CAMERA_MODE.load(Ordering::Relaxed);
        if camera_state == CameraState::On as i8 && camera_mode == 0 {
            if result > 0.0 {
                result = result.min(max_speed);
            }
            if result < 0.0 {
                result = result.max(max_speed * -1.0);
            }
        }
        return result;
    });

    //apply max speed to X orbit
    install_detour!(CAM_X_SPEED, cam_x_speed_va, |in_game_pad| {
        let mut result = CAM_X_SPEED.call(in_game_pad);
        let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
        let camera_mode = CAMERA_MODE.load(Ordering::Relaxed);
        if camera_state == CameraState::On as i8 {
            match camera_mode {
                0 => {
                    if result > 0.0 {
                        result = result.min(SPECTATE_ORBIT_X_SPEED);
                    }
                    if result < 0.0 {
                        result = result.max(SPECTATE_ORBIT_X_SPEED * -1.0);
                    }
                }
                1 => {
                    if result > 0.0 {
                        result = result.min(TOP_DOWN_ORBIT_X_SPEED);
                    }
                    if result < 0.0 {
                        result = result.max(TOP_DOWN_ORBIT_X_SPEED * -1.0);
                    }
                }
                _ => {}
            }
        }

        return result;
    });

    //get the item used as a trigger to start cam and mark center
    install_detour!(ITEM_USED, item_used_va, move |player_ins, item_id| {
        let Some(item_used) = (&*player_ins).chr_ins.tae_queued_use_item.param_id() else {
            ITEM_USED.call(player_ins, item_id);
            return;
        };
        let is_host =
            (&*player_ins).player_game_data.as_ref().multiplay_role == MultiplayRole::Host;
        let is_main_player = (&*player_ins).player_game_data.as_ref().is_main_player;
        let is_overworld = (&*player_ins).current_block_id.area() >= 60;
        //activate camera
        if item_used == CAMERA_ITEM_ID {
            if is_host && is_main_player {
                CAMERA_ACTIVATION_TRIGGER.store(true, Ordering::Relaxed);
            }
        }
        //set custom center pos
        if item_used == CAMERA_MARKER_ITEM_ID {
            let is_on = CAMERA_MARKER.load(Ordering::Relaxed);
            if is_host && is_main_player {
                if !is_on {
                    let player_pos = (&*player_ins).chr_ins.modules.physics.position;
                    save_marker_position_relative(player_pos);
                    display_net_message(
                        show_net_notice_va,
                        "A new camera center position has been set",
                    );
                    CAMERA_MARKER.store(true, Ordering::Relaxed);
                } else if is_on && !is_overworld {
                    write_lock_value(&MARKER_POS_LOCAL, ZEROED_POSITION);
                    display_net_message(
                        show_net_notice_va,
                        "The camera center position has been reset",
                    );
                    CAMERA_MARKER.store(false, Ordering::Relaxed);
                }
            }
            if !is_host {
                // log::info!("nothost");
            }
        }
        ITEM_USED.call(player_ins, item_id);
    });

    //use the get block coord fn to assign new center coords upon new block entry
    install_detour!(
        GET_BLOCK_COORDINATES,
        get_block_coords_va,
        move |world_block_info| {
            let result = GET_BLOCK_COORDINATES.call(world_block_info);
            let is_overworld = (*world_block_info).world_area_info_index == -1;
            if is_overworld {
                return result;
            }
            let current_center_coords = read_lock_value(&DEFAULT_POS);
            let new_center_coords =
                HavokPosition((*result).0, (*result).1, (*result).2, (*result).3);
            //check against old block
            if current_center_coords.0 != new_center_coords.0
                || current_center_coords.1 != new_center_coords.1
                || current_center_coords.2 != new_center_coords.2
            {
                write_lock_value(&DEFAULT_POS, new_center_coords);
            }

            return result;
        }
    );

    //remove cam on state on title screen
    install_detour!(
        TOP_TITLE_DIALOG,
        title_top_dialog_va,
        |param_1, param_2, param_3, param_4| {
            CAMERA_STATE.store(CameraState::Deactivating as i8, Ordering::Relaxed);
            let Ok(world_chr_man) = WorldChrMan::instance() else {
                return TOP_TITLE_DIALOG.call(param_1, param_2, param_3, param_4);
            };

            let players = world_chr_man.player_chr_set.characters();

            for player in players {
                if player.chr_ins.debug_flags.force_unloaded() {
                    // log::info!("force_unloaded detected, fixing...");
                    player.chr_ins.debug_flags.set_force_unloaded(false);
                }
            }
            return TOP_TITLE_DIALOG.call(param_1, param_2, param_3, param_4);
        }
    );

    //remove cam on state on stage load; use map init or other step instead
    install_detour!(STEP_MAP_INIT, step_map_init_va, move |ingame_step| {
        CAMERA_STATE.store(CameraState::Deactivating as i8, Ordering::Relaxed);
        write_lock_value(&MARKER_POS_LOCAL, ZEROED_POSITION);
        STEP_MAP_INIT.call(ingame_step);
    });

    //used to apply auto rotation to control movement
    install_detour!(
        APPLY_CONTROL_MOVEMENT,
        apply_control_movement_va,
        |chr_ex_follow_cam, delta_time, param_3| {
            APPLY_CONTROL_MOVEMENT.call(chr_ex_follow_cam, delta_time, param_3);

            let angles_euler_y = chr_ex_follow_cam.add(0x154) as *mut f32;
            let auto_rotation_speed = read_lock_value(&AUTO_ROTATE_CURRENT_SPEED);
            let distance = get_dist_offset_value().max(0.08);
            let speed_modifier = DIST_MIN / distance;
            let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
            let orbit_correction = ORBIT_CORRECTION.load(Ordering::Relaxed);

            if camera_state == (CameraState::On as i8) {
                if auto_rotation_speed != 0.0 {
                    *angles_euler_y += auto_rotation_speed * speed_modifier; //0.005;
                }
                if orbit_correction {
                    *angles_euler_y += ORBIT_CORRECTION_SPEED * speed_modifier;
                }
            }
        }
    );

    //stop applyzoomlerp from overwritting camera settings
    install_detour!(
        APPLY_ZOOM_LERP,
        apply_zoom_lerp_va,
        |param_1, param_2, param_3| {
            let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
            if camera_state == (CameraState::On as i8) {
                return;
            } else {
                APPLY_ZOOM_LERP.call(param_1, param_2, param_3);
            }
        }
    );

    //pad input reading for control
    install_detour!(POLL_INPUT, poll_input_va, move |param_1, input_id| {
        let is_valid_input = POLL_INPUT.call(param_1, input_id);
        let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
        let window_infocus = WINDOW_INFOCUS.load(Ordering::Relaxed);
        let window_infocus = WINDOW_INFOCUS.load(Ordering::Relaxed);
        if is_valid_input {
            // log::info!("input_id: {}", input_id);
        }

        if is_valid_input && camera_state == (CameraState::On as i8) && window_infocus {
            //check against input list
            if let Some(input) = InputId::check_id(input_id) {
                let now = now_ms();
                let last_time = LAST_INPUT_TIME_MS.load(Ordering::Relaxed);
                let last_input = CURRENT_INPUT.load(Ordering::Relaxed);
                let is_repeat = input_id == last_input && now - last_time < DEBOUNCE_MS;

                let camera_fov = read_lock_value(&FOV);
                let camera_mode = CAMERA_MODE.load(Ordering::Relaxed);
                let camera_dist_offset = get_dist_offset_value();
                let auto_transition = AUTO_TRANSITION.load(Ordering::Relaxed);

                let player_count = PLAYER_COUNT.load(Ordering::Relaxed) - 1;
                let current_plyr_index = PLAYER_INDEX.load(Ordering::Relaxed);

                //inputs captured multiple times per frame
                //fov and zoom
                match input {
                    //fov increase
                    InputId::DPadUp => {
                        if camera_fov > FOV_MIN {
                            write_lock_value(&FOV, camera_fov - 0.01);
                        }
                    }
                    //fov decrease
                    InputId::DPadDown => {
                        if camera_fov < FOV_MAX {
                            write_lock_value(&FOV, camera_fov + 0.01);
                        }
                    }
                    //distance decrease
                    InputId::PadR2 => {
                        if camera_mode != 2 {
                            if camera_dist_offset > DIST_MIN {
                                set_dist_offset_value(camera_dist_offset - 0.07);
                            }
                            // set_x_offset_value(camera_offset_x - 0.1);
                        }
                    }
                    //distance increase
                    InputId::PadL2 => {
                        if camera_mode != 2 {
                            if camera_dist_offset < DIST_MAX {
                                set_dist_offset_value(camera_dist_offset + 0.07);
                            }
                            // set_x_offset_value(camera_offset_x + 0.1);
                        }
                    }
                    _ => {}
                }

                //get inputs after time stagger to receive one per frame
                if !is_repeat {
                    LAST_INPUT_TIME_MS.store(now, Ordering::Relaxed);
                    CURRENT_INPUT.store(input_id, Ordering::Relaxed);

                    match input {
                        InputId::PadTri => {
                            if !auto_transition {
                                AUTO_TRANSITION.store(true, Ordering::Relaxed);
                                display_net_message(
                                    show_net_notice_va,
                                    "Auto-Transition Mode Enabled",
                                );
                            } else {
                                AUTO_ROTATE.store(false, Ordering::Relaxed);
                                write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_OFF);
                                AUTO_TRANSITION.store(false, Ordering::Relaxed);
                                AUTO_TRANSITION_NEXT.store(0, Ordering::Relaxed);
                                display_net_message(
                                    show_net_notice_va,
                                    "Auto-Transition Mode Disabled",
                                );
                            }
                        }
                        InputId::PadSq => {
                            let fps_boost_mode = FPS_BOOST.load(Ordering::Relaxed);
                            if !fps_boost_mode {
                                FPS_BOOST.store(true, Ordering::Relaxed);
                                display_net_message(show_net_notice_va, "FPS-Boost Mode Enabled");
                            } else {
                                FPS_BOOST.store(false, Ordering::Relaxed);
                                display_net_message(show_net_notice_va, "FPS-Boost Mode Disabled");
                            }
                        }
                        InputId::PadCir => {
                            let camera_collision = CAMERA_COLLISION.load(Ordering::Relaxed);
                            if !camera_collision {
                                CAMERA_COLLISION.store(true, Ordering::Relaxed);
                                display_net_message(show_net_notice_va, "Camera Collision Enabled");
                            } else {
                                CAMERA_COLLISION.store(false, Ordering::Relaxed);
                                display_net_message(
                                    show_net_notice_va,
                                    "Camera Collision Disabled",
                                );
                            }
                        }
                        InputId::PadX => {}
                        //cycle players increase
                        InputId::DPadRight => {
                            let increment = current_plyr_index + 1;
                            if increment.abs() > (player_count as i32) || increment.abs() < 1 {
                                PLAYER_INDEX.store(1, Ordering::Relaxed);
                            } else {
                                PLAYER_INDEX.store(increment, Ordering::Relaxed);
                            }
                        }
                        //cycle players decrease
                        InputId::DPadLeft => {
                            let increment = current_plyr_index - 1;
                            if increment.abs() > (player_count as i32) || increment.abs() < 1 {
                                PLAYER_INDEX.store(player_count as i32, Ordering::Relaxed);
                            } else {
                                PLAYER_INDEX.store(increment, Ordering::Relaxed);
                            }
                        }
                        //camera mode decrease
                        InputId::PadL1 => {
                            if !auto_transition {
                                let increment = camera_mode - 1;
                                if increment.abs() < 3 {
                                    CAMERA_MODE.store(increment, Ordering::Relaxed);
                                } else {
                                    CAMERA_MODE.store(0, Ordering::Relaxed);
                                }
                                set_dist_offset_value(DIST_MIN);
                            }
                        }
                        //camera mode increase
                        InputId::PadR1 => {
                            if !auto_transition {
                                let increment = camera_mode + 1;
                                if increment.abs() < 3 {
                                    CAMERA_MODE.store(increment, Ordering::Relaxed);
                                } else {
                                    CAMERA_MODE.store(0, Ordering::Relaxed);
                                }
                                set_dist_offset_value(DIST_MIN)
                            };
                        }
                        //auto rotation
                        InputId::PadL3 => {
                            let current_rot_speed = read_lock_value(&AUTO_ROTATE_CURRENT_SPEED);
                            if !auto_transition {
                                if current_rot_speed == AUTO_ROTATE_OFF {
                                    write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_CW);
                                    display_net_message(
                                        show_net_notice_va,
                                        "Auto-Rotation Mode Enabled",
                                    );
                                } else if current_rot_speed == AUTO_ROTATE_CW {
                                    write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_CCW);
                                } else if current_rot_speed == AUTO_ROTATE_CCW {
                                    write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_OFF);
                                    display_net_message(
                                        show_net_notice_va,
                                        "Auto-Rotation Mode Disabled",
                                    );
                                }
                            }
                        }
                        //exit camera
                        InputId::PadR3 => {
                            CAMERA_STATE.store(CameraState::Deactivating as i8, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                    // log::info!("Cam Mode Change: {:?}", CAMERA_MODE.load(Ordering::Relaxed));
                }
            }
        }
        is_valid_input
    });

    //resolve pointers for global lock man
    let global_lock_tgt_man_ptr = unsafe {
        //32 offset +3
        let rip_offset = std::ptr::read_unaligned(target_lock_man_va.add(3) as *const i32);
        let next_instr_va = target_lock_man_va.add(7);
        let target_va = next_instr_va.offset(rip_offset as isize);
        target_va as u64
    };

    let cs_task = unsafe { CSTaskImp::instance().map_err(InitError::Program)? };
    cs_task.run_recurring(
        move |_: &FD4TaskData| {
            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
                return;
            };

            //setup pointers for tgt lock man settings
            let global_lock_tgt_man_ptr = global_lock_tgt_man_ptr as *const *mut u8;
            let lock_tgt_man = unsafe { *global_lock_tgt_man_ptr };
            let is_rotation_locked = unsafe { lock_tgt_man.add(0x298d) };
            let is_targeting_enabled = unsafe { lock_tgt_man.add(0x298b) };

            //set a flag for the item use animation and if it completed
            if let Some(ref main_player) = world_chr_man.main_player {
                // let anim_active = CAMERA_ACTIVATION_ANIM.load(Ordering::Relaxed);
                let anim_queue = &main_player.chr_ins.modules.time_act.anim_queue;
                let is_in_anim = anim_queue.iter().any(|a| a.anim_id == 50560);
                if is_in_anim {
                    CAMERA_ACTIVATION_ANIM.store(true, Ordering::Relaxed);
                } else {
                    CAMERA_ACTIVATION_ANIM.store(false, Ordering::Relaxed);
                }
            }

            // let mut player_count = 0;
            let mut is_lobby_ready = false;
            if let Ok(session_man) = unsafe { CSSessionManager::instance() } {
                // player_count = session_man.players.len();
                // PLAYER_COUNT.store(player_count as i32, Ordering::Relaxed);
                is_lobby_ready = session_man.lobby_state == LobbyState::Host;
            };

            let player_count = world_chr_man.player_chr_set.characters().count();
            PLAYER_COUNT.store(player_count, Ordering::Relaxed);

            //camera states and triggers
            let camera_state = CAMERA_STATE.load(Ordering::Relaxed);
            // log::info!("camera_state: {}", camera_state);
            let camera_mode = CAMERA_MODE.load(Ordering::Relaxed).abs();
            let camera_triggered = CAMERA_ACTIVATION_TRIGGER.load(Ordering::Relaxed);
            let in_activation_anim = CAMERA_ACTIVATION_ANIM.load(Ordering::Relaxed);

            //camera on trigger
            if camera_state == (CameraState::Off as i8) && camera_triggered && !in_activation_anim {
                if let Some(ref mut main_player) = world_chr_man.main_player {
                    unsafe {
                        let is_host = main_player.player_game_data.as_ref().is_main_player;

                        let players_in_world = player_count > 1;

                        let bypass = CAMERA_BYPASS.load(Ordering::Relaxed);

                        //enable checks
                        if !is_host && !bypass {
                            display_net_message(
                                show_net_notice_va,
                                "Cannot start camera. You are not the host.",
                            );
                            CAMERA_ACTIVATION_TRIGGER.store(false, Ordering::Relaxed);
                            return;
                        } else if !players_in_world && !bypass {
                            display_net_message(
                                show_net_notice_va,
                                "Cannot start camera. No players in world.",
                            );
                            CAMERA_ACTIVATION_TRIGGER.store(false, Ordering::Relaxed);
                            return;
                        } else if !is_lobby_ready && !bypass {
                            display_net_message(
                                show_net_notice_va,
                                "Cannot start camera. Lobby is not ready.",
                            );
                            CAMERA_ACTIVATION_TRIGGER.store(false, Ordering::Relaxed);
                            return;
                        } else {
                            // set unload state
                            if !main_player.chr_ins.debug_flags.force_unloaded() {
                                main_player.chr_ins.debug_flags.set_force_unloaded(true);
                            }

                            main_player.chr_ins.chr_type = ChrType::MessageGhost;
                            let player_pos = main_player.chr_ins.modules.physics.position;

                            //set cam defaults
                            CAMERA_ACTIVATION_TRIGGER.store(false, Ordering::Relaxed);
                            CAMERA_STATE.store(CameraState::On as i8, Ordering::Relaxed);
                            PLAYER_INDEX.store(1, Ordering::Relaxed);
                            AUTO_ROTATE.store(false, Ordering::Relaxed);
                            AUTO_TRANSITION.store(false, Ordering::Relaxed);
                            AUTO_TRANSITION_NEXT.store(0, Ordering::Relaxed);
                            write_lock_value(&AUTO_ROTATE_CURRENT_SPEED, AUTO_ROTATE_OFF);
                            set_dist_offset_value(DIST_MIN);
                            write_lock_value(&FOV, 1.2);
                            save_return_position_relative(player_pos);
                            let custom_marker_on = CAMERA_MARKER.load(Ordering::Relaxed);
                            let is_overworld = main_player.current_block_id.area() >= 60;
                            if is_overworld && !custom_marker_on {
                                save_marker_position_relative(player_pos);
                                CAMERA_MARKER.store(true, Ordering::Relaxed);
                            }
                            main_player
                                .chr_ins
                                .modules
                                .action_request
                                .disabled_action_inputs
                                .set_use_item(true);
                        }
                    }
                };
            }

            //camera modes
            if camera_state == (CameraState::On as i8) {
                if camera_mode != 2 {
                    //remove orbit correction if on
                    let orbit_correction = ORBIT_CORRECTION.load(Ordering::Relaxed);
                    if orbit_correction {
                        ORBIT_CORRECTION.store(false, Ordering::Relaxed);
                    }
                }
                match camera_mode {
                    //spectate mode
                    0 => _spectate_mode(player_count),
                    //top-down mode
                    1 => _top_down_mode(),
                    //following mode
                    2 => _follow_mode(player_count, field_area_va, fade_to_color_va),
                    _ => {}
                }
            }

            //camera settings
            if camera_state == (CameraState::On as i8) {
                //lock on and hud properties
                lock_on_hud_props(lock_tgt_man, is_rotation_locked, is_targeting_enabled);
                //set camera properties; are reset on restore of detour
                camera_properties(camera_mode, cam_wall_collision_va);
                //auto transition mode
                if AUTO_TRANSITION.load(Ordering::Relaxed) {
                    _auto_transition_mode(player_count, camera_mode);
                }
                //fade logic
                start_dist_fade_logic(fade_to_color_va);
            }

            //deactivate camera
            if camera_state == (CameraState::Deactivating as i8) {
                start_camera_deactivation(
                    cam_wall_collision_va,
                    lock_tgt_man,
                    is_rotation_locked,
                    is_targeting_enabled,
                );
            }
        },
        CSTaskGroupIndex::FrameBegin,
    );
    Ok(())
}
