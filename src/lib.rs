//! Low-level FFI bindings to Netflix VMAF.
//!
//! All functions that call into libvmaf are unsafe. Consider using a safe
//! wrapper crate for application code.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

mod bindings;
pub use bindings::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version_constants() {
        assert!(VMAF_API_VERSION_MAJOR > 0);
    }

    #[test]
    fn test_vmaf_version() {
        unsafe {
            let version = vmaf_version();
            assert!(!version.is_null());
            assert!(!std::ffi::CStr::from_ptr(version).to_bytes().is_empty());
        }
    }

    #[test]
    fn test_context_lifecycle() {
        unsafe {
            let mut context = std::ptr::null_mut();
            let config = VmafConfiguration {
                log_level: VmafLogLevel_VMAF_LOG_LEVEL_NONE,
                n_threads: 0,
                n_subsample: 1,
                cpumask: 0,
                gpumask: 0,
            };

            assert_eq!(vmaf_init(&mut context, config), 0);
            assert!(!context.is_null());
            assert_eq!(vmaf_close(context), 0);
        }
    }
}
