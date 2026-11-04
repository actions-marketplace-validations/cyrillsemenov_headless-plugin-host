pub mod bundle;
pub mod component_handler;
pub mod contracts;
pub mod events_params;
pub mod gui;
pub mod host_app;
pub mod instance;
pub mod plug_frame;
pub mod stream;

pub use bundle::Vst3Bundle;
pub use instance::Vst3PluginInstance;

#[cfg(test)]
mod tests {
    use super::*;
    use hph_core::{PluginBundle, PluginFormat};
    use std::path::Path;

    #[test]
    fn test_vst3_bundle_trait_and_loader() {
        assert_eq!(Vst3Bundle::FORMAT, PluginFormat::Vst3);
        assert_eq!(Vst3Bundle::EXTENSIONS, &[".vst3"]);
        assert!(Vst3Bundle::matches_path(Path::new("/tmp/Test.vst3")));
        assert!(Vst3Bundle::matches_path(Path::new(
            "/tmp/Test.vst3/Contents/MacOS/Test"
        )));
        assert!(!Vst3Bundle::matches_path(Path::new("/tmp/Test.clap")));

        assert_eq!(Vst3Bundle::LOADER.format, PluginFormat::Vst3);
        assert!(Vst3Bundle::LOADER.matches_path(Path::new("/tmp/Test.vst3")));
    }

    #[test]
    fn test_vst3_bundle_create_instance_and_com_lifecycle_single_and_split() {
        use crate::bundle::{Vst3BundleInner, Vst3ClassEntry};
        use crate::host_app::Vst3HostApplication;
        use hph_core::PluginDescriptorInfo;
        use hph_vst3_sys::*;
        use std::ffi::c_void;
        use std::ptr;
        use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
        use std::sync::Arc;

        static FACTORY_REFS: AtomicU32 = AtomicU32::new(1);
        static COMP_REFS: AtomicU32 = AtomicU32::new(0);
        static CTRL_REFS: AtomicU32 = AtomicU32::new(0);
        static VIEW_REFS: AtomicU32 = AtomicU32::new(0);
        static COMP_TERMINATES: AtomicI32 = AtomicI32::new(0);
        static CTRL_TERMINATES: AtomicI32 = AtomicI32::new(0);
        static CP_DISCONNECTS: AtomicI32 = AtomicI32::new(0);
        static SPLIT_MODE: AtomicBool = AtomicBool::new(false);

        const TEST_CID: Steinberg_TUID = [1; 16];
        const TEST_CTRL_CID: Steinberg_TUID = [2; 16];

        unsafe extern "system" fn view_qi(
            _this: *mut c_void,
            _iid: *const Steinberg_TUID,
            obj: *mut *mut c_void,
        ) -> Steinberg_tresult {
            unsafe {
                if !obj.is_null() {
                    *obj = ptr::null_mut();
                }
            }
            kNoInterface
        }
        unsafe extern "system" fn view_add_ref(_this: *mut c_void) -> Steinberg_uint32 {
            VIEW_REFS.fetch_add(1, Ordering::SeqCst) + 1
        }
        unsafe extern "system" fn view_release(_this: *mut c_void) -> Steinberg_uint32 {
            VIEW_REFS.fetch_sub(1, Ordering::SeqCst) - 1
        }
        unsafe extern "system" fn view_is_platform_supported(
            _this: *mut c_void,
            _type: Steinberg_FIDString,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_attached(
            _this: *mut c_void,
            _parent: *mut c_void,
            _type: Steinberg_FIDString,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_removed(_this: *mut c_void) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_on_wheel(
            _this: *mut c_void,
            _distance: f32,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_on_key(
            _this: *mut c_void,
            _key: Steinberg_char16,
            _key_code: Steinberg_int16,
            _modifiers: Steinberg_int16,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_get_size(
            _this: *mut c_void,
            size: *mut Steinberg_ViewRect,
        ) -> Steinberg_tresult {
            unsafe {
                if !size.is_null() {
                    (*size).left = 0;
                    (*size).top = 0;
                    (*size).right = 640;
                    (*size).bottom = 480;
                }
            }
            kResultOk
        }
        unsafe extern "system" fn view_on_size(
            _this: *mut c_void,
            _new_size: *mut Steinberg_ViewRect,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_on_focus(
            _this: *mut c_void,
            _state: Steinberg_TBool,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_set_frame(
            _this: *mut c_void,
            _frame: *mut Steinberg_IPlugFrame,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn view_can_resize(_this: *mut c_void) -> Steinberg_tresult {
            kResultOk
        }

        static VIEW_VTBL: Steinberg_IPlugViewVtbl = Steinberg_IPlugViewVtbl {
            queryInterface: view_qi,
            addRef: view_add_ref,
            release: view_release,
            isPlatformTypeSupported: view_is_platform_supported,
            attached: view_attached,
            removed: view_removed,
            onWheel: view_on_wheel,
            onKeyDown: view_on_key,
            onKeyUp: view_on_key,
            getSize: view_get_size,
            onSize: view_on_size,
            onFocus: view_on_focus,
            setFrame: view_set_frame,
            canResize: view_can_resize,
            checkSizeConstraint: view_on_size,
        };
        static mut VIEW_OBJ: Steinberg_IPlugView = Steinberg_IPlugView {
            lpVtbl: &raw const VIEW_VTBL,
        };

        unsafe extern "system" fn cp_comp_add_ref(_this: *mut c_void) -> Steinberg_uint32 {
            COMP_REFS.fetch_add(1, Ordering::SeqCst) + 1
        }
        unsafe extern "system" fn cp_comp_release(_this: *mut c_void) -> Steinberg_uint32 {
            COMP_REFS.fetch_sub(1, Ordering::SeqCst) - 1
        }
        unsafe extern "system" fn cp_ctrl_add_ref(_this: *mut c_void) -> Steinberg_uint32 {
            CTRL_REFS.fetch_add(1, Ordering::SeqCst) + 1
        }
        unsafe extern "system" fn cp_ctrl_release(_this: *mut c_void) -> Steinberg_uint32 {
            CTRL_REFS.fetch_sub(1, Ordering::SeqCst) - 1
        }
        unsafe extern "system" fn cp_connect(
            _this: *mut c_void,
            _other: *mut Steinberg_Vst_IConnectionPoint,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn cp_disconnect(
            _this: *mut c_void,
            _other: *mut Steinberg_Vst_IConnectionPoint,
        ) -> Steinberg_tresult {
            CP_DISCONNECTS.fetch_add(1, Ordering::SeqCst);
            kResultOk
        }
        unsafe extern "system" fn cp_notify(
            _this: *mut c_void,
            _msg: *mut c_void,
        ) -> Steinberg_tresult {
            kResultOk
        }

        static COMP_CP_VTBL: Steinberg_Vst_IConnectionPointVtbl =
            Steinberg_Vst_IConnectionPointVtbl {
                queryInterface: view_qi,
                addRef: cp_comp_add_ref,
                release: cp_comp_release,
                connect: cp_connect,
                disconnect: cp_disconnect,
                notify: cp_notify,
            };
        static mut COMP_CP_OBJ: Steinberg_Vst_IConnectionPoint = Steinberg_Vst_IConnectionPoint {
            lpVtbl: &raw const COMP_CP_VTBL,
        };

        static CTRL_CP_VTBL: Steinberg_Vst_IConnectionPointVtbl =
            Steinberg_Vst_IConnectionPointVtbl {
                queryInterface: view_qi,
                addRef: cp_ctrl_add_ref,
                release: cp_ctrl_release,
                connect: cp_connect,
                disconnect: cp_disconnect,
                notify: cp_notify,
            };
        static mut CTRL_CP_OBJ: Steinberg_Vst_IConnectionPoint = Steinberg_Vst_IConnectionPoint {
            lpVtbl: &raw const CTRL_CP_VTBL,
        };

        unsafe extern "system" fn ctrl_qi(
            _this: *mut c_void,
            iid: *const Steinberg_TUID,
            obj: *mut *mut c_void,
        ) -> Steinberg_tresult {
            unsafe {
                if *iid == Steinberg_Vst_IEditController_iid || *iid == Steinberg_FUnknown_iid {
                    CTRL_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut CTRL_OBJ) as *mut c_void;
                    kResultOk
                } else if *iid == Steinberg_Vst_IConnectionPoint_iid {
                    CTRL_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut CTRL_CP_OBJ) as *mut c_void;
                    kResultOk
                } else {
                    *obj = ptr::null_mut();
                    kNoInterface
                }
            }
        }
        unsafe extern "system" fn ctrl_init(
            _this: *mut c_void,
            _ctx: *mut Steinberg_FUnknown,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn ctrl_terminate(_this: *mut c_void) -> Steinberg_tresult {
            CTRL_TERMINATES.fetch_add(1, Ordering::SeqCst);
            kResultOk
        }
        unsafe extern "system" fn ctrl_stream_op(
            _this: *mut c_void,
            _state: *mut Steinberg_IBStream,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn ctrl_get_param_count(_this: *mut c_void) -> Steinberg_int32 {
            0
        }
        unsafe extern "system" fn ctrl_get_param_info(
            _this: *mut c_void,
            _idx: Steinberg_int32,
            _info: *mut Steinberg_Vst_ParameterInfo,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn ctrl_get_param_str(
            _this: *mut c_void,
            _id: Steinberg_Vst_ParamID,
            _val: Steinberg_Vst_ParamValue,
            _str: *mut Steinberg_Vst_TChar,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn ctrl_get_param_val(
            _this: *mut c_void,
            _id: Steinberg_Vst_ParamID,
            _str: *const Steinberg_Vst_TChar,
            _val: *mut Steinberg_Vst_ParamValue,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn ctrl_param_conv(
            _this: *mut c_void,
            _id: Steinberg_Vst_ParamID,
            val: Steinberg_Vst_ParamValue,
        ) -> Steinberg_Vst_ParamValue {
            val
        }
        unsafe extern "system" fn ctrl_get_param_norm(
            _this: *mut c_void,
            _id: Steinberg_Vst_ParamID,
        ) -> Steinberg_Vst_ParamValue {
            0.0
        }
        unsafe extern "system" fn ctrl_set_param_norm(
            _this: *mut c_void,
            _id: Steinberg_Vst_ParamID,
            _val: Steinberg_Vst_ParamValue,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn ctrl_set_handler(
            _this: *mut c_void,
            _handler: *mut Steinberg_Vst_IComponentHandler,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn ctrl_create_view(
            _this: *mut c_void,
            _name: Steinberg_FIDString,
        ) -> *mut Steinberg_IPlugView {
            VIEW_REFS.fetch_add(1, Ordering::SeqCst);
            &raw mut VIEW_OBJ
        }

        static CTRL_VTBL: Steinberg_Vst_IEditControllerVtbl = Steinberg_Vst_IEditControllerVtbl {
            queryInterface: ctrl_qi,
            addRef: cp_ctrl_add_ref,
            release: cp_ctrl_release,
            initialize: ctrl_init,
            terminate: ctrl_terminate,
            setComponentState: ctrl_stream_op,
            setState: ctrl_stream_op,
            getState: ctrl_stream_op,
            getParameterCount: ctrl_get_param_count,
            getParameterInfo: ctrl_get_param_info,
            getParamStringByValue: ctrl_get_param_str,
            getParamValueByString: ctrl_get_param_val,
            normalizedParamToPlain: ctrl_param_conv,
            plainParamToNormalized: ctrl_param_conv,
            getParamNormalized: ctrl_get_param_norm,
            setParamNormalized: ctrl_set_param_norm,
            setComponentHandler: ctrl_set_handler,
            createView: ctrl_create_view,
        };
        static mut CTRL_OBJ: Steinberg_Vst_IEditController = Steinberg_Vst_IEditController {
            lpVtbl: &raw const CTRL_VTBL,
        };

        unsafe extern "system" fn proc_set_bus_arr(
            _this: *mut c_void,
            _ins: *mut Steinberg_Vst_SpeakerArrangement,
            _num_ins: Steinberg_int32,
            _outs: *mut Steinberg_Vst_SpeakerArrangement,
            _num_outs: Steinberg_int32,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn proc_get_bus_arr(
            _this: *mut c_void,
            _dir: Steinberg_Vst_BusDirection,
            _idx: Steinberg_int32,
            _arr: *mut Steinberg_Vst_SpeakerArrangement,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn proc_can_sample_size(
            _this: *mut c_void,
            _size: Steinberg_int32,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn proc_get_u32(_this: *mut c_void) -> Steinberg_uint32 {
            0
        }
        unsafe extern "system" fn proc_setup(
            _this: *mut c_void,
            _setup: *mut Steinberg_Vst_ProcessSetup,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn proc_set_bool(
            _this: *mut c_void,
            _state: Steinberg_TBool,
        ) -> Steinberg_tresult {
            kResultOk
        }
        unsafe extern "system" fn proc_process(
            _this: *mut c_void,
            _data: *mut Steinberg_Vst_ProcessData,
        ) -> Steinberg_tresult {
            kResultOk
        }

        static PROC_VTBL: Steinberg_Vst_IAudioProcessorVtbl = Steinberg_Vst_IAudioProcessorVtbl {
            queryInterface: view_qi,
            addRef: cp_comp_add_ref,
            release: cp_comp_release,
            setBusArrangements: proc_set_bus_arr,
            getBusArrangement: proc_get_bus_arr,
            canProcessSampleSize: proc_can_sample_size,
            getLatencySamples: proc_get_u32,
            setupProcessing: proc_setup,
            setProcessing: proc_set_bool,
            process: proc_process,
            getTailSamples: proc_get_u32,
        };
        static mut PROC_OBJ: Steinberg_Vst_IAudioProcessor = Steinberg_Vst_IAudioProcessor {
            lpVtbl: &raw const PROC_VTBL,
        };

        unsafe extern "system" fn comp_qi(
            _this: *mut c_void,
            iid: *const Steinberg_TUID,
            obj: *mut *mut c_void,
        ) -> Steinberg_tresult {
            unsafe {
                if *iid == Steinberg_Vst_IComponent_iid || *iid == Steinberg_FUnknown_iid {
                    COMP_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut COMP_OBJ) as *mut c_void;
                    kResultOk
                } else if *iid == Steinberg_Vst_IAudioProcessor_iid {
                    COMP_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut PROC_OBJ) as *mut c_void;
                    kResultOk
                } else if *iid == Steinberg_Vst_IEditController_iid
                    && !SPLIT_MODE.load(Ordering::SeqCst)
                {
                    CTRL_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut CTRL_OBJ) as *mut c_void;
                    kResultOk
                } else if *iid == Steinberg_Vst_IConnectionPoint_iid
                    && SPLIT_MODE.load(Ordering::SeqCst)
                {
                    COMP_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut COMP_CP_OBJ) as *mut c_void;
                    kResultOk
                } else {
                    *obj = ptr::null_mut();
                    kNoInterface
                }
            }
        }
        unsafe extern "system" fn comp_terminate(_this: *mut c_void) -> Steinberg_tresult {
            COMP_TERMINATES.fetch_add(1, Ordering::SeqCst);
            kResultOk
        }
        unsafe extern "system" fn comp_get_ctrl_cid(
            _this: *mut c_void,
            cid: *mut Steinberg_TUID,
        ) -> Steinberg_tresult {
            if SPLIT_MODE.load(Ordering::SeqCst) {
                unsafe {
                    *cid = TEST_CTRL_CID;
                }
                kResultOk
            } else {
                kNoInterface
            }
        }
        unsafe extern "system" fn comp_get_bus_count(
            _this: *mut c_void,
            _media: Steinberg_Vst_MediaType,
            _dir: Steinberg_Vst_BusDirection,
        ) -> Steinberg_int32 {
            0
        }
        unsafe extern "system" fn comp_get_bus_info(
            _this: *mut c_void,
            _media: Steinberg_Vst_MediaType,
            _dir: Steinberg_Vst_BusDirection,
            _idx: Steinberg_int32,
            _bus: *mut Steinberg_Vst_BusInfo,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn comp_get_routing_info(
            _this: *mut c_void,
            _in_info: *mut Steinberg_Vst_RoutingInfo,
            _out_info: *mut Steinberg_Vst_RoutingInfo,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn comp_activate_bus(
            _this: *mut c_void,
            _media: Steinberg_Vst_MediaType,
            _dir: Steinberg_Vst_BusDirection,
            _idx: Steinberg_int32,
            _state: Steinberg_TBool,
        ) -> Steinberg_tresult {
            kResultOk
        }

        static COMP_VTBL: Steinberg_Vst_IComponentVtbl = Steinberg_Vst_IComponentVtbl {
            queryInterface: comp_qi,
            addRef: cp_comp_add_ref,
            release: cp_comp_release,
            initialize: ctrl_init,
            terminate: comp_terminate,
            getControllerClassId: comp_get_ctrl_cid,
            setIoMode: proc_can_sample_size,
            getBusCount: comp_get_bus_count,
            getBusInfo: comp_get_bus_info,
            getRoutingInfo: comp_get_routing_info,
            activateBus: comp_activate_bus,
            setActive: proc_set_bool,
            setState: ctrl_stream_op,
            getState: ctrl_stream_op,
        };
        static mut COMP_OBJ: Steinberg_Vst_IComponent = Steinberg_Vst_IComponent {
            lpVtbl: &raw const COMP_VTBL,
        };

        unsafe extern "system" fn factory_add_ref(_this: *mut c_void) -> Steinberg_uint32 {
            FACTORY_REFS.fetch_add(1, Ordering::SeqCst) + 1
        }
        unsafe extern "system" fn factory_release(_this: *mut c_void) -> Steinberg_uint32 {
            FACTORY_REFS.fetch_sub(1, Ordering::SeqCst) - 1
        }
        unsafe extern "system" fn factory_get_info(
            _this: *mut c_void,
            _info: *mut Steinberg_PFactoryInfo,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn factory_get_class_info(
            _this: *mut c_void,
            _idx: Steinberg_int32,
            _info: *mut Steinberg_PClassInfo,
        ) -> Steinberg_tresult {
            kNoInterface
        }
        unsafe extern "system" fn factory_create_instance(
            _this: *mut c_void,
            cid: Steinberg_FIDString,
            _iid: Steinberg_FIDString,
            obj: *mut *mut c_void,
        ) -> Steinberg_tresult {
            unsafe {
                let cid_slice = std::slice::from_raw_parts(cid as *const u8, 16);
                if cid_slice == TEST_CID {
                    COMP_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut COMP_OBJ) as *mut c_void;
                    kResultOk
                } else if cid_slice == TEST_CTRL_CID {
                    CTRL_REFS.fetch_add(1, Ordering::SeqCst);
                    *obj = (&raw mut CTRL_OBJ) as *mut c_void;
                    kResultOk
                } else {
                    *obj = ptr::null_mut();
                    kNoInterface
                }
            }
        }

        static FACTORY_VTBL: Steinberg_IPluginFactoryVtbl = Steinberg_IPluginFactoryVtbl {
            queryInterface: view_qi,
            addRef: factory_add_ref,
            release: factory_release,
            getFactoryInfo: factory_get_info,
            countClasses: ctrl_get_param_count,
            getClassInfo: factory_get_class_info,
            createInstance: factory_create_instance,
        };
        static mut FACTORY_OBJ: Steinberg_IPluginFactory = Steinberg_IPluginFactory {
            lpVtbl: &raw const FACTORY_VTBL,
        };

        let factory = unsafe { ComPtr::from_raw(&raw mut FACTORY_OBJ).unwrap() };
        let bundle = Vst3Bundle {
            inner: Arc::new(Vst3BundleInner {
                factory: Some(factory),
                host_app: Vst3HostApplication::new_boxed(),
                classes: vec![Vst3ClassEntry {
                    cid: TEST_CID,
                    descriptor: PluginDescriptorInfo {
                        format: PluginFormat::Vst3,
                        id: "01010101010101010101010101010101".into(),
                        name: "SyntheticVST3".into(),
                        vendor: "Test".into(),
                        url: String::new(),
                        manual_url: String::new(),
                        support_url: String::new(),
                        version: "1.0".into(),
                        description: String::new(),
                        features: vec![],
                    },
                }],
                bundle_exit_fn: None,
                #[cfg(target_os = "macos")]
                cf_bundle_ref: ptr::null_mut(),
                _library: None,
            }),
        };

        // 1. Single-component mode
        SPLIT_MODE.store(false, Ordering::SeqCst);
        {
            let mut inst = bundle.create_instance(None).unwrap();
            assert!(inst.supports_gui());
            inst.gui_create().unwrap();
            assert_eq!(VIEW_REFS.load(Ordering::SeqCst), 1);
            assert_eq!(inst.gui_get_size().unwrap(), (640, 480));
            let probes = inst.probe_extensions();
            assert!(!probes.is_empty());
            let checks = inst.run_extended_contract_checks().unwrap();
            assert!(!checks.is_empty());
            inst.gui_destroy();
            assert_eq!(VIEW_REFS.load(Ordering::SeqCst), 0);
        }
        // In single-component mode, component.terminate() is called once, controller.terminate() is not called,
        // and all references (component, processor, controller) are released.
        assert_eq!(COMP_TERMINATES.load(Ordering::SeqCst), 1);
        assert_eq!(CTRL_TERMINATES.load(Ordering::SeqCst), 0);
        assert_eq!(COMP_REFS.load(Ordering::SeqCst), 0);
        assert_eq!(CTRL_REFS.load(Ordering::SeqCst), 0);

        // 2. Split-component mode (separate component + controller + connection points)
        SPLIT_MODE.store(true, Ordering::SeqCst);
        {
            let mut inst = bundle.create_instance(None).unwrap();
            assert_eq!(COMP_REFS.load(Ordering::SeqCst), 3); // component + processor + comp_cp
            assert_eq!(CTRL_REFS.load(Ordering::SeqCst), 2); // controller + ctrl_cp
            inst.gui_create().unwrap();
            assert_eq!(VIEW_REFS.load(Ordering::SeqCst), 1);
            // Let Drop clean up plug_view, connection points, controller, processor, and component
        }
        assert_eq!(VIEW_REFS.load(Ordering::SeqCst), 0);
        assert_eq!(CP_DISCONNECTS.load(Ordering::SeqCst), 2);
        assert_eq!(COMP_TERMINATES.load(Ordering::SeqCst), 2);
        assert_eq!(CTRL_TERMINATES.load(Ordering::SeqCst), 1);
        assert_eq!(COMP_REFS.load(Ordering::SeqCst), 0);
        assert_eq!(CTRL_REFS.load(Ordering::SeqCst), 0);

        drop(bundle);
        assert_eq!(FACTORY_REFS.load(Ordering::SeqCst), 0);
    }
}
