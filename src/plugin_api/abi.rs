use std::ffi::{c_char, c_void};

pub const SIMENGINE_API_VERSION: u32 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum SimLogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SimContext {
    pub user_data: *mut c_void,

    pub log: extern "C" fn(user_data: *mut c_void, level: SimLogLevel, message: *const c_char),

    pub set_output: extern "C" fn(
        user_data: *mut c_void,
        name: *const c_char,
        payload: *const u8,
        payload_len: usize,
    ),

    pub get_input: extern "C" fn(
        user_data: *mut c_void,
        name: *const c_char,
        out_payload: *mut u8,
        out_payload_len: usize,
    ) -> usize,
}

#[repr(C)]
pub struct SimApi {
    pub api_version: u32,
    pub create: extern "C" fn(ctx: SimContext, config_json: *const c_char) -> *mut c_void,
    pub pre_step: extern "C" fn(instance: *mut c_void, dt_seconds: f64),
    pub step: extern "C" fn(instance: *mut c_void, dt_seconds: f64),
    pub post_step: extern "C" fn(instance: *mut c_void, dt_seconds: f64),
    pub destroy: extern "C" fn(instance: *mut c_void),
}

pub type GetSimApiFn = unsafe extern "C" fn() -> SimApi;
