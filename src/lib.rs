#![recursion_limit = "10000"]
use std::{thread::spawn, time::Duration};

use eldenring::util::system::{SystemInitError, wait_for_system_init};
use fromsoftware_shared::{
    F32Vector4, FromStatic, InstanceError, SharedTaskImpExt, program::Program,
};
use thiserror::Error;

// use
mod cam;
mod config;
mod rva;

pub fn init() {
    spawn(move || {
        let program = Program::current();
        wait_for_system_init(&program, Duration::MAX).unwrap();
    });
}

pub unsafe extern "C" fn dll_main(_hmodule: usize, reason: u32) -> bool {
    if reason == 1 {
        std::thread::spawn(move || {
            config::load();
            let program = Program::current();
            cam::hook(&program).expect("Could not apply camera hook");
        });
        init();
    }
    true
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("Could not find instruction pattern or found multiple instances. pattern = {0}")]
    FlakyPattern(&'static str),

    #[error("Could not convert between RVA and VA. {0}")]
    AddressConversion(pelite::Error),

    #[error("Could not convert find import. import = {0}")]
    MissingImport(&'static str),

    #[error("Retour error. {0}")]
    Retour(#[from] retour::Error),

    #[error("CsTaskImp error. {0}")]
    CsTaskImp(SystemInitError),

    #[error("Instance error. {0}")]
    Program(InstanceError),
}
