use crate::instance::AuPluginInstance;
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::bundle::{CFBundleCreate, CFBundleGetValueForInfoDictionaryKey, CFBundleRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::CFURL;
use hph_au_sys::*;
use hph_core::*;
use libloading::Library;
use objc2::encode::{Encoding, RefEncode};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;

#[repr(C)]
pub(crate) struct ObjCAudioUnitRecord {
    _private: [u8; 0],
}

unsafe impl RefEncode for ObjCAudioUnitRecord {
    const ENCODING_REF: Encoding =
        Encoding::Pointer(&Encoding::Struct("ComponentInstanceRecord", &[]));
}

#[derive(Debug, Clone)]
pub struct AuComponentEntry {
    pub desc: AudioComponentDescription,
    pub component: AudioComponent,
    pub descriptor: PluginDescriptorInfo,
    pub factory_function: Option<String>,
}

unsafe impl Send for AuComponentEntry {}
unsafe impl Sync for AuComponentEntry {}

pub(crate) struct AuBundleInner {
    pub bundle_path: PathBuf,
    _library: Option<Library>,
    pub(crate) entries: Vec<AuComponentEntry>,
}

unsafe impl Send for AuBundleInner {}
unsafe impl Sync for AuBundleInner {}

#[derive(Clone)]
pub struct AuBundle {
    pub(crate) inner: Arc<AuBundleInner>,
}

fn format_au_version(ver: u32) -> String {
    let major = (ver >> 16) & 0xFFFF;
    let minor = (ver >> 8) & 0xFF;
    let bugfix = ver & 0xFF;
    format!("{major}.{minor}.{bugfix}")
}

fn features_for_component_type(comp_type: OSType) -> Vec<String> {
    if comp_type == kAudioUnitType_Effect || comp_type == kAudioUnitType_OfflineEffect {
        vec!["audio-effect".into(), "auv2".into()]
    } else if comp_type == kAudioUnitType_MusicDevice {
        vec!["instrument".into(), "synthesizer".into(), "auv2".into()]
    } else if comp_type == kAudioUnitType_MusicEffect {
        vec![
            "audio-effect".into(),
            "instrument".into(),
            "midi-effect".into(),
            "auv2".into(),
        ]
    } else if comp_type == kAudioUnitType_Generator {
        vec!["generator".into(), "auv2".into()]
    } else if comp_type == kAudioUnitType_MIDIProcessor {
        vec!["note-effect".into(), "midi-effect".into(), "auv2".into()]
    } else {
        vec![fourcc_to_string(comp_type), "auv2".into()]
    }
}

fn split_vendor_and_name(raw_name: &str, fallback_name: &str) -> (String, String) {
    if let Some((vendor, name)) = raw_name.split_once(':') {
        let v = vendor.trim();
        let n = name.trim();
        if !v.is_empty() && !n.is_empty() {
            return (v.to_string(), n.to_string());
        }
    }
    ("Unknown Vendor".to_string(), fallback_name.to_string())
}

fn dict_get_string(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<String> {
    let k = CFString::new(key);
    let val = dict.find(&k)?;
    let cf_str = val.downcast::<CFString>()?;
    Some(cf_str.to_string())
}

fn dict_get_u32(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<u32> {
    let k = CFString::new(key);
    let val = dict.find(&k)?;
    if let Some(num) = val.downcast::<CFNumber>() {
        if let Some(v) = num.to_i64() {
            return Some(v as u32);
        }
    }
    if let Some(s) = val.downcast::<CFString>() {
        let st = s.to_string();
        if let Some(hex) = st.strip_prefix("0x").or_else(|| st.strip_prefix("0X")) {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                return Some(v);
            }
        }
        if let Ok(v) = st.parse::<u32>() {
            return Some(v);
        }
    }
    None
}

fn resolve_component_bundle_root(path: &Path) -> PathBuf {
    let mut cur = Some(path);
    while let Some(p) = cur {
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("component"))
            .unwrap_or(false)
        {
            return p.to_path_buf();
        }
        cur = p.parent();
    }
    path.to_path_buf()
}

fn resolve_component_binary(bundle_root: &Path, executable_name: Option<&str>) -> Option<PathBuf> {
    let macos_dir = bundle_root.join("Contents").join("MacOS");
    if let Some(exec) = executable_name {
        let candidate = macos_dir.join(exec);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(stem) = bundle_root.file_stem() {
        let candidate = macos_dir.join(stem);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(entries) = std::fs::read_dir(&macos_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

impl AuBundle {
    pub const FORMAT: PluginFormat = PluginFormat::AuV2;
    pub const EXTENSIONS: &'static [&'static str] = &[".component"];
    pub const LOADER: FormatLoader = FormatLoader::new::<Self>(Self::FORMAT, Self::EXTENSIONS);

    pub fn bundle_path(&self) -> &Path {
        &self.inner.bundle_path
    }
}

impl PluginBundle for AuBundle {
    fn loader() -> FormatLoader {
        Self::LOADER
    }

    fn load(path: &Path) -> HostResult<Arc<Self>> {
        let bundle_root = resolve_component_bundle_root(path);
        if !bundle_root.exists() {
            return Err(HostError::LoadError(format!(
                "AU component path does not exist: {}",
                bundle_root.display()
            )));
        }

        let cf_url = CFURL::from_path(&bundle_root, true).ok_or_else(|| {
            HostError::LoadError(format!(
                "Failed to construct CFURL for {}",
                bundle_root.display()
            ))
        })?;

        let cf_bundle: CFBundleRef =
            unsafe { CFBundleCreate(ptr::null(), cf_url.as_concrete_TypeRef()) };
        if cf_bundle.is_null() {
            return Err(HostError::LoadError(format!(
                "CFBundleCreate failed for {}",
                bundle_root.display()
            )));
        }

        let exec_key = CFString::from_static_string("CFBundleExecutable");
        let exec_name = unsafe {
            let val =
                CFBundleGetValueForInfoDictionaryKey(cf_bundle, exec_key.as_concrete_TypeRef());
            if !val.is_null() {
                let cf_str = CFString::wrap_under_get_rule(val as CFStringRef);
                Some(cf_str.to_string())
            } else {
                None
            }
        };

        let bundle_name_key = CFString::from_static_string("CFBundleName");
        let fallback_bundle_name = unsafe {
            let val = CFBundleGetValueForInfoDictionaryKey(
                cf_bundle,
                bundle_name_key.as_concrete_TypeRef(),
            );
            if !val.is_null() {
                CFString::wrap_under_get_rule(val as CFStringRef).to_string()
            } else {
                bundle_root
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("AudioUnit")
                    .to_string()
            }
        };

        let binary_path = resolve_component_binary(&bundle_root, exec_name.as_deref());
        let library = if let Some(ref bin) = binary_path {
            unsafe { Library::new(bin).ok() }
        } else {
            None
        };

        let comps_key = CFString::from_static_string("AudioComponents");
        let comps_ptr = unsafe {
            CFBundleGetValueForInfoDictionaryKey(cf_bundle, comps_key.as_concrete_TypeRef())
        };
        if comps_ptr.is_null() {
            unsafe { CFRelease(cf_bundle as CFTypeRef) };
            return Err(HostError::LoadError(format!(
                "Bundle {} Info.plist is missing 'AudioComponents' array",
                bundle_root.display()
            )));
        }

        let comps_array: CFArray<CFDictionary<CFString, CFType>> =
            unsafe { CFArray::wrap_under_get_rule(comps_ptr as CFArrayRef) };

        let mut entries = Vec::new();
        for idx in 0..comps_array.len() {
            let Some(item_ref) = comps_array.get(idx) else {
                continue;
            };
            let dict = &*item_ref;

            let Some(type_str) = dict_get_string(dict, "type") else {
                continue;
            };
            let Some(subtype_str) = dict_get_string(dict, "subtype") else {
                continue;
            };
            let Some(manuf_str) = dict_get_string(dict, "manufacturer") else {
                continue;
            };

            let Some(comp_type) = str_to_fourcc(&type_str) else {
                continue;
            };
            let Some(comp_subtype) = str_to_fourcc(&subtype_str) else {
                continue;
            };
            let Some(comp_manuf) = str_to_fourcc(&manuf_str) else {
                continue;
            };

            let raw_name =
                dict_get_string(dict, "name").unwrap_or_else(|| fallback_bundle_name.clone());
            let description =
                dict_get_string(dict, "description").unwrap_or_else(|| raw_name.clone());
            let version_int = dict_get_u32(dict, "version").unwrap_or(0x00010000);
            let factory_fn_name = dict_get_string(dict, "factoryFunction");

            let desc = AudioComponentDescription {
                componentType: comp_type,
                componentSubType: comp_subtype,
                componentManufacturer: comp_manuf,
                componentFlags: 0,
                componentFlagsMask: 0,
            };

            let mut component: AudioComponent = ptr::null_mut();

            // 1. Try direct in-process registration via factoryFunction from loaded Mach-O binary
            if let (Some(ref lib), Some(ref fn_name)) = (&library, &factory_fn_name) {
                unsafe {
                    if let Ok(sym) = lib.get::<AudioComponentFactoryFunction>(fn_name.as_bytes()) {
                        let cf_name = CFString::new(&raw_name);
                        let existing = AudioComponentFindNext(ptr::null_mut(), &desc);
                        if !existing.is_null() {
                            component = existing;
                        } else {
                            let reg = AudioComponentRegister(
                                &desc,
                                cf_name.as_concrete_TypeRef(),
                                version_int,
                                *sym,
                            );
                            if !reg.is_null() {
                                component = reg;
                            }
                        }
                    }
                }
            }

            // 2. Fallback to system AudioComponentFindNext
            if component.is_null() {
                component = unsafe { AudioComponentFindNext(ptr::null_mut(), &desc) };
            }

            if component.is_null() {
                continue;
            }

            let (vendor, clean_name) = split_vendor_and_name(&raw_name, &fallback_bundle_name);
            let id = format!(
                "{}:{}:{}",
                fourcc_to_string(comp_type),
                fourcc_to_string(comp_subtype),
                fourcc_to_string(comp_manuf)
            );

            entries.push(AuComponentEntry {
                desc,
                component,
                descriptor: PluginDescriptorInfo {
                    format: PluginFormat::AuV2,
                    id,
                    name: clean_name,
                    vendor,
                    version: format_au_version(version_int),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description,
                    features: features_for_component_type(comp_type),
                },
                factory_function: factory_fn_name,
            });
        }

        unsafe { CFRelease(cf_bundle as CFTypeRef) };

        if entries.is_empty() {
            return Err(HostError::LoadError(format!(
                "No valid AudioComponents could be registered or found in {}",
                bundle_root.display()
            )));
        }

        Ok(Arc::new(Self {
            inner: Arc::new(AuBundleInner {
                bundle_path: bundle_root,
                _library: library,
                entries,
            }),
        }))
    }

    fn format(&self) -> PluginFormat {
        Self::FORMAT
    }

    fn descriptors(&self) -> Vec<PluginDescriptorInfo> {
        self.inner
            .entries
            .iter()
            .map(|e| e.descriptor.clone())
            .collect()
    }

    fn create_instance(&self, plugin_id: Option<&str>) -> HostResult<Box<dyn PluginInstance>> {
        let entry = if let Some(req_id) = plugin_id {
            self.inner
                .entries
                .iter()
                .find(|e| {
                    e.descriptor.id.eq_ignore_ascii_case(req_id) || e.descriptor.name == req_id
                })
                .ok_or_else(|| {
                    HostError::InitError(format!("AudioUnit ID '{req_id}' not found in bundle"))
                })?
        } else {
            &self.inner.entries[0]
        };

        let mut unit: AudioUnit = ptr::null_mut();
        let status = unsafe { AudioComponentInstanceNew(entry.component, &mut unit) };
        if status != noErr || unit.is_null() {
            return Err(HostError::InitError(format!(
                "AudioComponentInstanceNew failed with OSStatus {status}"
            )));
        }

        Ok(Box::new(AuPluginInstance::new(
            Arc::clone(&self.inner),
            entry.clone(),
            unit,
        )?))
    }
}
