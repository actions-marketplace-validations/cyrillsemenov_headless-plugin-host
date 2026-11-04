#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

pub mod base;
pub mod gui;
pub mod iid;
pub mod interfaces;
pub mod vst_types;

pub use base::*;
pub use gui::*;
pub use iid::*;
pub use interfaces::*;
pub use vst_types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_vst3_abi_layouts() {
        assert_eq!(std::mem::size_of::<Steinberg_TUID>(), 16);
        assert_eq!(std::mem::size_of::<Steinberg_ViewRect>(), 16);
        assert_eq!(std::mem::size_of::<Steinberg_PClassInfo>(), 116);
        assert_eq!(std::mem::size_of::<Steinberg_PClassInfo2>(), 440);
        // FUnknown IID check on non-Windows
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            Steinberg_FUnknown_iid,
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x46
            ]
        );
    }

    #[test]
    fn verify_com_ptr_query_deref_into_raw_and_drop() {
        use std::ffi::c_void;
        use std::ptr;
        use std::sync::atomic::{AtomicU32, Ordering};

        static REF_COUNT: AtomicU32 = AtomicU32::new(1);

        unsafe extern "system" fn mock_query_interface(
            this: *mut c_void,
            iid: *const Steinberg_TUID,
            obj: *mut *mut c_void,
        ) -> Steinberg_tresult {
            unsafe {
                if *iid == Steinberg_Vst_IProcessContextRequirements::IID {
                    REF_COUNT.fetch_add(1, Ordering::SeqCst);
                    *obj = this;
                    kResultOk
                } else if *iid == Steinberg_Vst_IMidiMapping::IID {
                    // Buggy plugin edge case: returns kResultOk (0) but leaves *obj = null
                    *obj = ptr::null_mut();
                    kResultOk
                } else {
                    *obj = ptr::null_mut();
                    kNoInterface
                }
            }
        }

        unsafe extern "system" fn mock_add_ref(_this: *mut c_void) -> u32 {
            REF_COUNT.fetch_add(1, Ordering::SeqCst) + 1
        }

        unsafe extern "system" fn mock_release(_this: *mut c_void) -> u32 {
            REF_COUNT.fetch_sub(1, Ordering::SeqCst) - 1
        }

        unsafe extern "system" fn mock_get_reqs(_this: *mut c_void) -> u32 {
            0x42
        }

        static VTBL: Steinberg_Vst_IProcessContextRequirementsVtbl =
            Steinberg_Vst_IProcessContextRequirementsVtbl {
                queryInterface: mock_query_interface,
                addRef: mock_add_ref,
                release: mock_release,
                getProcessContextRequirements: mock_get_reqs,
            };

        let mut instance = Steinberg_Vst_IProcessContextRequirements { lpVtbl: &VTBL };
        REF_COUNT.store(1, Ordering::SeqCst);

        unsafe {
            // 1. Null source pointer and null vtable pointer return None
            assert!(
                ComPtr::<Steinberg_Vst_IProcessContextRequirements>::query(ptr::null_mut::<
                    Steinberg_FUnknown,
                >())
                .is_none()
            );
            assert!(
                ComPtr::<Steinberg_Vst_IProcessContextRequirements>::from_raw(ptr::null_mut())
                    .is_none()
            );
            let mut null_vtbl_inst = Steinberg_Vst_IProcessContextRequirements {
                lpVtbl: ptr::null(),
            };
            assert!(
                ComPtr::<Steinberg_Vst_IProcessContextRequirements>::from_raw(&mut null_vtbl_inst)
                    .is_none()
            );
            assert!(ComPtr::<Steinberg_Vst_IProcessContextRequirements>::query(
                &mut null_vtbl_inst
            )
            .is_none());

            // 2. Unimplemented interface or buggy kResultOk + null output returns None without leaking refcount
            assert!(
                ComPtr::<Steinberg_Vst_IAudioPresentationLatency>::query(&mut instance).is_none()
            );
            assert!(ComPtr::<Steinberg_Vst_IMidiMapping>::query(&mut instance).is_none());
            assert_eq!(REF_COUNT.load(Ordering::SeqCst), 1);

            // 3. Supported interface increments refcount on query, derefs to vtable, and decrements on Drop
            {
                let req = ComPtr::<Steinberg_Vst_IProcessContextRequirements>::query(&mut instance)
                    .expect("Should query IProcessContextRequirements");
                assert_eq!(REF_COUNT.load(Ordering::SeqCst), 2);
                let flags = (req.getProcessContextRequirements)(req.as_void());
                assert_eq!(flags, 0x42);
            }
            assert_eq!(REF_COUNT.load(Ordering::SeqCst), 1);

            // 4. into_raw transfers ownership without calling release on Drop
            let raw = {
                let req = ComPtr::<Steinberg_Vst_IProcessContextRequirements>::query(&mut instance)
                    .unwrap();
                assert_eq!(REF_COUNT.load(Ordering::SeqCst), 2);
                req.into_raw()
            };
            assert_eq!(REF_COUNT.load(Ordering::SeqCst), 2);
            let reclaimed =
                ComPtr::<Steinberg_Vst_IProcessContextRequirements>::from_raw(raw).unwrap();
            drop(reclaimed);
            assert_eq!(REF_COUNT.load(Ordering::SeqCst), 1);
        }
    }
}
