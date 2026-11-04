use crate::instance::AuPluginInstance;
use core_foundation::base::{CFRelease, CFTypeRef};
use hph_au_sys::*;
use hph_core::*;
use std::ffi::c_void;
use std::ptr;

impl PluginContractChecks for AuPluginInstance {
    fn probe_extensions(&self) -> Vec<ExtensionProbeResult> {
        let mut results = Vec::new();

        // 1. AudioUnit Component Instance
        results.push(ExtensionProbeResult {
            name: "AudioComponentInstance".into(),
            supported: !self.unit.is_null(),
            valid_vtable: !self.unit.is_null(),
            notes: format!(
                "type='{}', subtype='{}', manufacturer='{}'",
                fourcc_to_string(self.entry.desc.componentType),
                fourcc_to_string(self.entry.desc.componentSubType),
                fourcc_to_string(self.entry.desc.componentManufacturer)
            ),
        });

        // 2. Parameters
        results.push(ExtensionProbeResult {
            name: "kAudioUnitProperty_ParameterList".into(),
            supported: !self.cached_params.is_empty(),
            valid_vtable: true,
            notes: format!("{} parameters", self.cached_params.len()),
        });

        // 3. ClassInfo State Serialization
        let has_state = self.supports_state();
        results.push(ExtensionProbeResult {
            name: "kAudioUnitProperty_ClassInfo".into(),
            supported: has_state,
            valid_vtable: has_state,
            notes: "CFPropertyList state dictionary".into(),
        });

        // 4. Cocoa UI
        let has_gui = self.supports_gui();
        results.push(ExtensionProbeResult {
            name: "kAudioUnitProperty_CocoaUI".into(),
            supported: has_gui,
            valid_vtable: has_gui,
            notes: if has_gui {
                "AUCocoaUIBase NSView".into()
            } else {
                "No Cocoa UI".into()
            },
        });

        // 5. SupportedNumChannels
        let mut ch_size: u32 = 0;
        let mut writable: Boolean = 0;
        let has_ch_info = unsafe {
            AudioUnitGetPropertyInfo(
                self.unit,
                kAudioUnitProperty_SupportedNumChannels,
                kAudioUnitScope_Global,
                0,
                &mut ch_size,
                &mut writable,
            ) == noErr
                && ch_size >= (std::mem::size_of::<AUChannelInfo>() as u32)
        };
        results.push(ExtensionProbeResult {
            name: "kAudioUnitProperty_SupportedNumChannels".into(),
            supported: has_ch_info,
            valid_vtable: true,
            notes: format!(
                "{} explicit AUChannelInfo configurations",
                ch_size as usize / std::mem::size_of::<AUChannelInfo>()
            ),
        });

        // 6. FactoryPresets & PresentPreset
        unsafe {
            let mut presets_arr: CFTypeRef = ptr::null();
            let mut arr_size = std::mem::size_of::<CFTypeRef>() as u32;
            let has_presets = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_FactoryPresets,
                kAudioUnitScope_Global,
                0,
                &mut presets_arr as *mut _ as *mut c_void,
                &mut arr_size,
            ) == noErr
                && !presets_arr.is_null();
            let mut preset_count = 0isize;
            if has_presets {
                preset_count = CFArrayGetCount(presets_arr);
                CFRelease(presets_arr);
            }
            results.push(ExtensionProbeResult {
                name: "kAudioUnitProperty_FactoryPresets".into(),
                supported: has_presets,
                valid_vtable: true,
                notes: if has_presets {
                    format!("{preset_count} factory presets")
                } else {
                    "No factory presets".into()
                },
            });

            // 7. BypassEffect
            let mut bypass_val: u32 = 0;
            let mut bypass_size = std::mem::size_of::<u32>() as u32;
            let has_bypass = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_BypassEffect,
                kAudioUnitScope_Global,
                0,
                &mut bypass_val as *mut _ as *mut c_void,
                &mut bypass_size,
            ) == noErr;
            results.push(ExtensionProbeResult {
                name: "kAudioUnitProperty_BypassEffect".into(),
                supported: has_bypass,
                valid_vtable: true,
                notes: if has_bypass {
                    format!("Native AUv2 bypass (current={bypass_val})")
                } else {
                    "Not supported".into()
                },
            });

            // 8. ElementCount (Input & Output Buses)
            let mut in_buses: u32 = 0;
            let mut out_buses: u32 = 0;
            let mut u32_size = std::mem::size_of::<u32>() as u32;
            let _ = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ElementCount,
                kAudioUnitScope_Input,
                0,
                &mut in_buses as *mut _ as *mut c_void,
                &mut u32_size,
            );
            u32_size = std::mem::size_of::<u32>() as u32;
            let _ = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ElementCount,
                kAudioUnitScope_Output,
                0,
                &mut out_buses as *mut _ as *mut c_void,
                &mut u32_size,
            );
            results.push(ExtensionProbeResult {
                name: "kAudioUnitProperty_ElementCount".into(),
                supported: in_buses > 0 || out_buses > 0,
                valid_vtable: true,
                notes: format!("{in_buses} input bus(es), {out_buses} output bus(es)"),
            });

            // 9. InPlaceProcessing
            let mut in_place: u32 = 0;
            u32_size = std::mem::size_of::<u32>() as u32;
            let has_in_place = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_InPlaceProcessing,
                kAudioUnitScope_Global,
                0,
                &mut in_place as *mut _ as *mut c_void,
                &mut u32_size,
            ) == noErr;
            results.push(ExtensionProbeResult {
                name: "kAudioUnitProperty_InPlaceProcessing".into(),
                supported: has_in_place,
                valid_vtable: true,
                notes: if has_in_place {
                    format!("in_place={}", in_place != 0)
                } else {
                    "Not supported".into()
                },
            });
        }

        results
    }

    fn run_extended_contract_checks(&mut self) -> HostResult<Vec<String>> {
        let mut verified = Vec::new();
        unsafe {
            // 1. FactoryPresets & PresentPreset cycling
            let saved_state = self.save_state().ok();
            let mut presets_arr: CFTypeRef = ptr::null();
            let mut arr_size = std::mem::size_of::<CFTypeRef>() as u32;
            if AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_FactoryPresets,
                kAudioUnitScope_Global,
                0,
                &mut presets_arr as *mut _ as *mut c_void,
                &mut arr_size,
            ) == noErr
                && !presets_arr.is_null()
            {
                let count = CFArrayGetCount(presets_arr);
                let mut loaded_count = 0usize;
                for idx in 0..count.min(4) {
                    let preset_ptr = CFArrayGetValueAtIndex(presets_arr, idx) as *const AUPreset;
                    if !preset_ptr.is_null() {
                        let preset = *preset_ptr;
                        if AudioUnitSetProperty(
                            self.unit,
                            kAudioUnitProperty_PresentPreset,
                            kAudioUnitScope_Global,
                            0,
                            &preset as *const _ as *const c_void,
                            std::mem::size_of::<AUPreset>() as u32,
                        ) == noErr
                        {
                            loaded_count += 1;
                        }
                    }
                }
                CFRelease(presets_arr);
                verified.push(format!(
                    "kAudioUnitProperty_FactoryPresets ({count} total, {loaded_count} loaded via PresentPreset)"
                ));
            }
            if let Some(st) = saved_state {
                let _ = self.load_state(&st);
            }

            // 2. Native kAudioUnitProperty_BypassEffect toggle & readback
            let mut orig_bypass: u32 = 0;
            let mut bypass_size = std::mem::size_of::<u32>() as u32;
            if AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_BypassEffect,
                kAudioUnitScope_Global,
                0,
                &mut orig_bypass as *mut _ as *mut c_void,
                &mut bypass_size,
            ) == noErr
            {
                let on_val: u32 = 1;
                let _ = AudioUnitSetProperty(
                    self.unit,
                    kAudioUnitProperty_BypassEffect,
                    kAudioUnitScope_Global,
                    0,
                    &on_val as *const _ as *const c_void,
                    std::mem::size_of::<u32>() as u32,
                );
                let mut readback: u32 = 0;
                bypass_size = std::mem::size_of::<u32>() as u32;
                let _ = AudioUnitGetProperty(
                    self.unit,
                    kAudioUnitProperty_BypassEffect,
                    kAudioUnitScope_Global,
                    0,
                    &mut readback as *mut _ as *mut c_void,
                    &mut bypass_size,
                );
                let _ = AudioUnitSetProperty(
                    self.unit,
                    kAudioUnitProperty_BypassEffect,
                    kAudioUnitScope_Global,
                    0,
                    &orig_bypass as *const _ as *mut c_void,
                    std::mem::size_of::<u32>() as u32,
                );
                verified.push(format!(
                    "kAudioUnitProperty_BypassEffect (readback={readback})"
                ));
            }

            // 3. InPlaceProcessing property check
            let mut in_place: u32 = 0;
            let mut u32_size = std::mem::size_of::<u32>() as u32;
            if AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_InPlaceProcessing,
                kAudioUnitScope_Global,
                0,
                &mut in_place as *mut _ as *mut c_void,
                &mut u32_size,
            ) == noErr
            {
                verified.push(format!("kAudioUnitProperty_InPlaceProcessing ({in_place})"));
            }
        }
        Ok(verified)
    }
}
