use crate::{plugin_api::SimContext, plugin_api::SimLogLevel};
use std::ffi::CString;

/// Clean API exposed to simulation authors.
#[derive(Debug, Copy, Clone)]
pub struct SimulationContext {
    raw: SimContext,
}

impl SimulationContext {
    pub fn from_raw(raw: SimContext) -> Self {
        Self { raw }
    }

    pub fn log(&self, level: SimLogLevel, message: impl AsRef<str>) {
        let message = CString::new(message.as_ref()).expect("log message contained a NUL byte");
        (self.raw.log)(self.raw.user_data, level, message.as_ptr());
    }

    pub fn info(&self, message: impl AsRef<str>) {
        self.log(SimLogLevel::Info, message);
    }

    pub fn set_output_bytes(&self, name: impl AsRef<str>, payload: &[u8]) {
        let name = CString::new(name.as_ref()).expect("output name contained a NUL byte");
        (self.raw.set_output)(
            self.raw.user_data,
            name.as_ptr(),
            payload.as_ptr(),
            payload.len(),
        );
    }

    pub fn set_output_f32(&self, name: impl AsRef<str>, value: f32) {
        self.set_output_bytes(name, &value.to_le_bytes());
    }

    pub fn get_input_bytes(&self, name: impl AsRef<str>, out: &mut [u8]) -> Option<usize> {
        let name = CString::new(name.as_ref()).expect("input name contained a NUL byte");
        let bytes_read = (self.raw.get_input)(
            self.raw.user_data,
            name.as_ptr(),
            out.as_mut_ptr(),
            out.len(),
        );

        if bytes_read == 0 {
            None
        } else {
            Some(bytes_read)
        }
    }

    pub fn get_input_f32(&self, name: impl AsRef<str>) -> Option<f32> {
        self.get_fixed_input(name).map(f32::from_le_bytes)
    }

    fn get_fixed_input<const N: usize>(&self, name: impl AsRef<str>) -> Option<[u8; N]> {
        let mut buffer = [0u8; N];
        match self.get_input_bytes(name, &mut buffer) {
            Some(bytes_read) if bytes_read == N => Some(buffer),
            _ => None,
        }
    }
}
