use crate::component_handler::{comp_handler_query_interface, Vst3ComponentHandler};
use crate::host_app::host_app_create_instance;
use crate::instance::Vst3PluginInstance;
use hph_core::*;
use hph_vst3_sys::*;
use std::ffi::{c_void, CStr};
use std::ptr;

impl PluginContractChecks for Vst3PluginInstance {
    fn probe_extensions(&self) -> Vec<ExtensionProbeResult> {
        let mut results = Vec::new();
        let comp_probes: &[(&str, Steinberg_TUID)] = &[
            ("IComponent", Steinberg_Vst_IComponent_iid),
            ("IAudioProcessor", Steinberg_Vst_IAudioProcessor_iid),
            (
                "IAudioPresentationLatency",
                Steinberg_Vst_IAudioPresentationLatency_iid,
            ),
            (
                "IProcessContextRequirements",
                Steinberg_Vst_IProcessContextRequirements_iid,
            ),
            (
                "IConnectionPoint (Component)",
                Steinberg_Vst_IConnectionPoint_iid,
            ),
        ];

        for &(name, ref iid) in comp_probes {
            let supported = unsafe {
                ComPtr::<Steinberg_FUnknown>::query_iid(self.component.as_ptr(), iid).is_some()
            };
            let mut notes = if supported {
                "Implemented on IComponent".to_string()
            } else {
                "Not implemented".to_string()
            };
            if name == "IProcessContextRequirements" {
                if let Some(pcr) = unsafe {
                    ComPtr::<Steinberg_Vst_IProcessContextRequirements>::query(
                        self.component.as_ptr(),
                    )
                } {
                    let flags = unsafe { (pcr.getProcessContextRequirements)(pcr.as_void()) };
                    notes = format!("ProcessContext flags: 0x{flags:04X}");
                }
            }
            results.push(ExtensionProbeResult {
                name: name.to_string(),
                supported,
                valid_vtable: supported,
                notes,
            });
        }

        let ctrl_probes: &[(&str, Steinberg_TUID)] = &[
            ("IEditController", Steinberg_Vst_IEditController_iid),
            ("IEditController2", Steinberg_Vst_IEditController2_iid),
            ("IUnitInfo", Steinberg_Vst_IUnitInfo_iid),
            ("IMidiMapping", Steinberg_Vst_IMidiMapping_iid),
            (
                "INoteExpressionController",
                Steinberg_Vst_INoteExpressionController_iid,
            ),
            (
                "IKeyswitchController",
                Steinberg_Vst_IKeyswitchController_iid,
            ),
        ];

        let ctrl_target = if self.is_single_component {
            Some(self.component.as_void())
        } else {
            self.controller.as_ref().map(|c| c.as_void())
        };

        for &(name, ref iid) in ctrl_probes {
            let supported = ctrl_target.is_some_and(|target| unsafe {
                ComPtr::<Steinberg_FUnknown>::query_iid(target, iid).is_some()
            });
            let mut notes = if supported {
                if self.is_single_component {
                    "Single-component controller".to_string()
                } else {
                    "Split-component controller".to_string()
                }
            } else {
                "Not implemented".to_string()
            };
            if let (true, Some(target)) = (supported, ctrl_target) {
                unsafe {
                    match name {
                        "IUnitInfo" => {
                            if let Some(ui) = ComPtr::<Steinberg_Vst_IUnitInfo>::query(target) {
                                let units = (ui.getUnitCount)(ui.as_void());
                                let lists = (ui.getProgramListCount)(ui.as_void());
                                notes = format!("{units} units, {lists} program lists");
                            }
                        }
                        "IMidiMapping" => {
                            if let Some(mm) = ComPtr::<Steinberg_Vst_IMidiMapping>::query(target) {
                                let mut pid: Steinberg_Vst_ParamID = 0;
                                let mapped = (mm.getMidiControllerAssignment)(
                                    mm.as_void(),
                                    0,
                                    0,
                                    1,
                                    &mut pid,
                                ) == kResultOk;
                                notes = format!("CC#1 mapped: {mapped}");
                            }
                        }
                        "INoteExpressionController" => {
                            if let Some(nec) =
                                ComPtr::<Steinberg_Vst_INoteExpressionController>::query(target)
                            {
                                let cnt = (nec.getNoteExpressionCount)(nec.as_void(), 0, 0);
                                notes = format!("{cnt} note expressions");
                            }
                        }
                        "IKeyswitchController" => {
                            if let Some(ksc) =
                                ComPtr::<Steinberg_Vst_IKeyswitchController>::query(target)
                            {
                                let cnt = (ksc.getKeyswitchCount)(ksc.as_void(), 0, 0);
                                notes = format!("{cnt} keyswitches");
                            }
                        }
                        _ => {}
                    }
                }
            }
            results.push(ExtensionProbeResult {
                name: name.to_string(),
                supported,
                valid_vtable: supported,
                notes,
            });
        }

        let supports_f64 = unsafe {
            (self.processor.canProcessSampleSize)(
                self.processor.as_void(),
                Steinberg_Vst_SymbolicSampleSizes_kSample64,
            ) == kResultOk
        };
        results.push(ExtensionProbeResult {
            name: "IAudioProcessor::kSample64".into(),
            supported: supports_f64,
            valid_vtable: true,
            notes: if supports_f64 {
                "Supports 64-bit double precision DSP".into()
            } else {
                "32-bit float DSP only".into()
            },
        });

        results
    }

    fn run_extended_contract_checks(&mut self) -> HostResult<Vec<String>> {
        let mut verified = Vec::new();
        if self.is_active {
            self.deactivate();
        }

        unsafe {
            // 1. Verify Host IMessage + IAttributeList factory via IHostApplication::createInstance
            let mut msg_ptr: *mut c_void = ptr::null_mut();
            let res = host_app_create_instance(
                ptr::null_mut(),
                &Steinberg_Vst_IMessage_iid,
                &Steinberg_Vst_IMessage_iid,
                &mut msg_ptr,
            );
            if res == kResultOk {
                if let Some(msg) = ComPtr::<Steinberg_Vst_IMessage>::from_raw(
                    msg_ptr as *mut Steinberg_Vst_IMessage,
                ) {
                    (msg.setMessageID)(msg.as_void(), c"HPH_Ping".as_ptr());
                    let id_ptr = (msg.getMessageID)(msg.as_void());
                    let id_str = if id_ptr.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(id_ptr).to_string_lossy().into_owned()
                    };
                    let attrs = (msg.getAttributes)(msg.as_void());
                    if !attrs.is_null() {
                        let attr_void = attrs as *mut c_void;
                        let _ = ((*(*attrs).lpVtbl).setInt)(attr_void, c"test_int".as_ptr(), 42);
                        let mut out_int: i64 = 0;
                        let _ = ((*(*attrs).lpVtbl).getInt)(
                            attr_void,
                            c"test_int".as_ptr(),
                            &mut out_int,
                        );
                        let _ =
                            ((*(*attrs).lpVtbl).setFloat)(attr_void, c"test_float".as_ptr(), 0.75);
                        let mut out_flt: f64 = 0.0;
                        let _ = ((*(*attrs).lpVtbl).getFloat)(
                            attr_void,
                            c"test_float".as_ptr(),
                            &mut out_flt,
                        );
                        if id_str == "HPH_Ping" && out_int == 42 && (out_flt - 0.75).abs() < 1e-9 {
                            verified.push(
                                "IHostApplication::createInstance (IMessage + IAttributeList)"
                                    .into(),
                            );
                        }
                    }
                }
            }

            // 2. Verify IComponentHandler2 aggregation
            let handler_ptr = &mut *self._comp_handler as *mut Vst3ComponentHandler as *mut c_void;
            let mut ch2_ptr: *mut c_void = ptr::null_mut();
            if comp_handler_query_interface(
                handler_ptr,
                &Steinberg_Vst_IComponentHandler2_iid,
                &mut ch2_ptr,
            ) == kResultOk
            {
                if let Some(ch2) = ComPtr::<Steinberg_Vst_IComponentHandler2>::from_raw(
                    ch2_ptr as *mut Steinberg_Vst_IComponentHandler2,
                ) {
                    let _ = (ch2.startGroupEdit)(ch2.as_void());
                    let _ = (ch2.setDirty)(ch2.as_void(), 1);
                    let _ = (ch2.finishGroupEdit)(ch2.as_void());
                    verified.push("IComponentHandler2 (setDirty & groupEdit)".into());
                }
            }

            // 3. IAudioPresentationLatency on IComponent (while deactivated)
            if let Some(apl) =
                ComPtr::<Steinberg_Vst_IAudioPresentationLatency>::query(self.component.as_ptr())
            {
                let _ = (apl.setAudioPresentationLatencySamples)(
                    apl.as_void(),
                    Steinberg_Vst_BusDirections_kOutput,
                    0,
                    0,
                );
                verified.push("IAudioPresentationLatency".into());
            }

            // 4. IProcessContextRequirements on IComponent
            if let Some(pcr) =
                ComPtr::<Steinberg_Vst_IProcessContextRequirements>::query(self.component.as_ptr())
            {
                let flags = (pcr.getProcessContextRequirements)(pcr.as_void());
                verified.push(format!("IProcessContextRequirements (0x{flags:04X})"));
            }

            // 5. Controller extensions: IEditController2, IUnitInfo, IMidiMapping
            let ctrl_target = if self.is_single_component {
                Some(self.component.as_void())
            } else {
                self.controller.as_ref().map(|c| c.as_void())
            };
            if let Some(target) = ctrl_target {
                if let Some(ec2) = ComPtr::<Steinberg_Vst_IEditController2>::query(target) {
                    let _ = (ec2.setKnobMode)(ec2.as_void(), 0);
                    verified.push("IEditController2 (setKnobMode)".into());
                }

                if let Some(ui) = ComPtr::<Steinberg_Vst_IUnitInfo>::query(target) {
                    let unit_cnt = (ui.getUnitCount)(ui.as_void());
                    for i in 0..unit_cnt.max(0) {
                        let mut uinfo: Steinberg_Vst_UnitInfo = std::mem::zeroed();
                        let _ = (ui.getUnitInfo)(ui.as_void(), i, &mut uinfo);
                    }
                    let list_cnt = (ui.getProgramListCount)(ui.as_void());
                    for i in 0..list_cnt.max(0) {
                        let mut plinfo: Steinberg_Vst_ProgramListInfo = std::mem::zeroed();
                        let _ = (ui.getProgramListInfo)(ui.as_void(), i, &mut plinfo);
                    }
                    verified.push(format!("IUnitInfo ({unit_cnt} units, {list_cnt} lists)"));
                }

                if let Some(mm) = ComPtr::<Steinberg_Vst_IMidiMapping>::query(target) {
                    let mut mapped_count = 0usize;
                    for cc in [1i16, 7, 10, 11, 64, 74] {
                        let mut pid: Steinberg_Vst_ParamID = 0;
                        if (mm.getMidiControllerAssignment)(mm.as_void(), 0, 0, cc, &mut pid)
                            == kResultOk
                        {
                            mapped_count += 1;
                        }
                    }
                    verified.push(format!("IMidiMapping ({mapped_count} standard CCs mapped)"));
                }
            }
        }

        Ok(verified)
    }
}
