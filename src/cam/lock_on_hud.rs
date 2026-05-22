use eldenring::cs::{CSFeManHudState, CSFeManImp};
use fromsoftware_shared::FromStatic;

pub fn lock_on_hud_props(
    lock_tgt_man: *mut u8,
    is_rotation_locked: *mut u8,
    is_targeting_enabled: *mut u8,
) {
    unsafe {
        //hud hide
        if let Ok(fe_man) = CSFeManImp::instance_mut() {
            if fe_man.hud_state != CSFeManHudState::HideAll {
                fe_man.hud_state = CSFeManHudState::HideAll;
            }
        }

        //set target lock settings
        if !lock_tgt_man.is_null() {
            //enable rotation
            *is_rotation_locked = 0;
            //disable targetr switching
            *is_targeting_enabled = 0;
            //
            // *is_manual_lock_enabled = 1;
        }
    }
}
