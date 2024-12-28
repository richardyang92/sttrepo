#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SherpaHandle {
    pub recognizer: *const (),
    pub stream: *const (),
}

unsafe impl Send for SherpaHandle { }
unsafe impl Sync for SherpaHandle { }

#[link(name = "sherpa-bridge")]
extern "C" {
    fn sherpa_init(
        tokens: *const ::std::os::raw::c_char,
        encoder: *const ::std::os::raw::c_char,
        decoder: *const ::std::os::raw::c_char,
        joiner: *const ::std::os::raw::c_char) -> SherpaHandle;
    fn sherpa_transcribe(
            handle: SherpaHandle,
            result: *mut ::std::os::raw::c_char,
            samples: *const f32,
            len: ::std::os::raw::c_int);
    fn sherpa_reset(handle: SherpaHandle);
    fn sherpa_close(handle: SherpaHandle);
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Sherpa {
    handle: Option<SherpaHandle>,
}

impl Sherpa {
    pub(crate) fn new() -> Self {
        Self { handle: None }
    }
}

impl Sherpa {
    pub fn init(&mut self, tokens: &str, encoder: &str, decoder: &str, joiner: &str) {
        let tokens_cstr = ::std::ffi::CString::new(tokens).unwrap();
        let encoder_cstr = ::std::ffi::CString::new(encoder).unwrap();
        let decoder_cstr = ::std::ffi::CString::new(decoder).unwrap();
        let joiner_cstr = ::std::ffi::CString::new(joiner).unwrap();

        self.handle.replace(unsafe {
            sherpa_init(tokens_cstr.as_ptr(),
                encoder_cstr.as_ptr(),
                decoder_cstr.as_ptr(),
                joiner_cstr.as_ptr())
        });
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        if let Some(handle) = self.handle {
            let len = samples.len() as i32;
            let mut result_buf = [0_u8; 2048];
            let result_ptr = result_buf.as_mut_ptr() as *mut ::std::os::raw::c_char;  

            unsafe { sherpa_transcribe(handle, result_ptr, samples.as_ptr(), len) };

            let c_str = unsafe { ::std::ffi::CStr::from_ptr(result_ptr) };
            Ok(c_str.to_str().unwrap_or("").to_string())
        } else {
            Err("transcribe: No handle found".to_string())
        }
    }

    pub fn reset(&self) -> Result<(), String> {
        if let Some(handle) = self.handle {
            unsafe { sherpa_reset(handle) };
            Ok(())
        } else {
            Err("reset: No handle found".to_string())
        }
    }

    pub fn close(&self) -> Result<(), String> {
        if let Some(handle) = self.handle {
            unsafe { sherpa_close(handle) };
            Ok(())
        } else {
            Err("close: No handle found".to_string())
        }
    }
}