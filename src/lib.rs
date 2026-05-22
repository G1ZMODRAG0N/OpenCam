#![recursion_limit = "10000"]
use fromsoftware_shared::Program;

// use
mod cam;
mod config;
mod rva;

pub unsafe extern "C" fn dll_main(_hmodule: usize, reason: u32) -> bool {
    if reason == 1 {
        std::thread::spawn(move || {
            config::load();
            let program = Program::current();
            cam::hook(&program).expect("Could not apply camera hook");
        });
    }
    true
}
