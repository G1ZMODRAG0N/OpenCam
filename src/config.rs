use std::{
    result::Result::Ok,
    sync::atomic::{AtomicBool, Ordering},
};

pub static SHOW_PLAYER_PKT_LOSS: AtomicBool = AtomicBool::new(false);
pub static SHOW_PLAYER_LOCATION: AtomicBool = AtomicBool::new(true);

fn get_bool(value: Option<&Option<String>>) -> bool {
    return value
        .and_then(|inner| inner.as_deref())
        .map(|val| val.to_lowercase() == "true")
        .unwrap_or(false);
}

pub fn load() {
    //get settings file
    let settings_file = ini::ini!("./config.ini");

    //get sections
    // let host_features = settings_file.get("host_features").unwrap();
    // let game_settings = settings_file.get("game_settings").unwrap();

    //host features
    // DISABLE_LOCK_ON.store(
    //     get_bool(host_features.get("disable_lock_on")),
    //     Ordering::Relaxed,
    // );
}
