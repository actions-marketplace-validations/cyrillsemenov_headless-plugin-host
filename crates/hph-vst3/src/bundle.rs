use crate::component_handler::{SharedHostState, Vst3ComponentHandler};
use crate::host_app::Vst3HostApplication;
use crate::instance::Vst3PluginInstance;
use crate::plug_frame::Vst3PlugFrame;
use crate::stream::Vst3MemoryStream;
use hph_core::*;
use hph_vst3_sys::*;
use libloading::Library;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct Vst3ClassEntry {
    pub(crate) cid: Steinberg_TUID,
    pub(crate) descriptor: PluginDescriptorInfo,
}

pub(crate) struct Vst3BundleInner {
    pub(crate) factory: Option<ComPtr<Steinberg_IPluginFactory>>,
    pub(crate) host_app: Box<Vst3HostApplication>,
    pub(crate) classes: Vec<Vst3ClassEntry>,
    pub(crate) bundle_exit_fn: Option<unsafe extern "C" fn() -> bool>,
    #[cfg(target_os = "macos")]
    pub(crate) cf_bundle_ref: *mut c_void,
    pub(crate) _library: Option<Arc<Library>>,
}

unsafe impl Send for Vst3BundleInner {}
unsafe impl Sync for Vst3BundleInner {}

impl Drop for Vst3BundleInner {
    fn drop(&mut self) {
        // Release IPluginFactory before invoking bundleExit/ExitDll/ModuleExit
        drop(self.factory.take());
        unsafe {
            if let Some(exit_fn) = self.bundle_exit_fn {
                let _ = exit_fn();
            }
            #[cfg(target_os = "macos")]
            if !self.cf_bundle_ref.is_null() {
                core_foundation::base::CFRelease(self.cf_bundle_ref);
                self.cf_bundle_ref = ptr::null_mut();
            }
        }
        // Keep the shared library mapped so detached plugin threads and registered
        // Objective-C classes / C++ atexit handlers never jump into unmapped pages.
        std::mem::forget(self._library.take());
    }
}

#[derive(Clone)]
pub struct Vst3Bundle {
    pub(crate) inner: Arc<Vst3BundleInner>,
}

impl Vst3Bundle {
    pub const FORMAT: PluginFormat = PluginFormat::Vst3;
    pub const EXTENSIONS: &'static [&'static str] = &[".vst3"];
    pub const LOADER: FormatLoader = FormatLoader::new::<Self>(Self::FORMAT, Self::EXTENSIONS);
}

impl PluginBundle for Vst3Bundle {
    fn loader() -> FormatLoader
    where
        Self: Sized,
    {
        Self::LOADER
    }

    fn load(path: &Path) -> HostResult<Arc<Self>>
    where
        Self: Sized,
    {
        let binary_path = resolve_vst3_binary_path(path)?;
        let library = unsafe {
            Library::new(&binary_path).map_err(|e| {
                HostError::LoadError(format!(
                    "Failed to dlopen VST3 binary '{}': {}",
                    binary_path.display(),
                    e
                ))
            })?
        };

        #[cfg(target_os = "macos")]
        let cf_bundle_ref = create_macos_cf_bundle(path);

        #[cfg(target_os = "macos")]
        let bundle_exit_fn = unsafe {
            if let Ok(bundle_entry) =
                library.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"bundleEntry\0")
            {
                if !bundle_entry(cf_bundle_ref) {
                    return Err(HostError::InitError(
                        "VST3 bundleEntry() returned false".into(),
                    ));
                }
            }
            library
                .get::<unsafe extern "C" fn() -> bool>(b"bundleExit\0")
                .ok()
                .map(|sym| *sym)
        };

        #[cfg(target_os = "windows")]
        let bundle_exit_fn = unsafe {
            if let Ok(init_dll) = library.get::<unsafe extern "C" fn() -> bool>(b"InitDll\0") {
                if !init_dll() {
                    return Err(HostError::InitError("VST3 InitDll() returned false".into()));
                }
            }
            library
                .get::<unsafe extern "C" fn() -> bool>(b"ExitDll\0")
                .ok()
                .map(|sym| *sym)
        };

        #[cfg(target_os = "linux")]
        let bundle_exit_fn = unsafe {
            if let Ok(mod_entry) =
                library.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"ModuleEntry\0")
            {
                if !mod_entry(ptr::null_mut()) {
                    return Err(HostError::InitError(
                        "VST3 ModuleEntry() returned false".into(),
                    ));
                }
            }
            library
                .get::<unsafe extern "C" fn() -> bool>(b"ModuleExit\0")
                .ok()
                .map(|sym| *sym)
        };

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        let bundle_exit_fn: Option<unsafe extern "C" fn() -> bool> = None;

        let get_factory_fn = unsafe {
            library
                .get::<unsafe extern "system" fn() -> *mut Steinberg_IPluginFactory>(
                    b"GetPluginFactory\0",
                )
                .map_err(|e| {
                    HostError::LoadError(format!("Missing GetPluginFactory symbol: {}", e))
                })?
        };

        let factory = unsafe { ComPtr::from_raw(get_factory_fn()) }
            .ok_or_else(|| HostError::LoadError("GetPluginFactory() returned null".into()))?;

        let mut host_app = Vst3HostApplication::new_boxed();

        unsafe {
            if let Some(factory3) = ComPtr::<Steinberg_IPluginFactory3>::query(factory.as_ptr()) {
                let _ = (factory3.setHostContext)(
                    factory3.as_void(),
                    &mut *host_app as *mut Vst3HostApplication as *mut Steinberg_FUnknown,
                );
            }
        }

        let classes = unsafe { enumerate_vst3_classes(&factory) };
        if classes.is_empty() {
            return Err(HostError::LoadError(
                "VST3 factory contains no 'Audio Module Class' entries".into(),
            ));
        }

        Ok(Arc::new(Self {
            inner: Arc::new(Vst3BundleInner {
                factory: Some(factory),
                host_app,
                classes,
                bundle_exit_fn,
                #[cfg(target_os = "macos")]
                cf_bundle_ref,
                _library: Some(Arc::new(library)),
            }),
        }))
    }

    fn format(&self) -> PluginFormat {
        Self::FORMAT
    }

    fn descriptors(&self) -> Vec<PluginDescriptorInfo> {
        self.inner
            .classes
            .iter()
            .map(|c| c.descriptor.clone())
            .collect()
    }

    fn create_instance(&self, plugin_id: Option<&str>) -> HostResult<Box<dyn PluginInstance>> {
        let entry = match plugin_id {
            Some(id) => self
                .inner
                .classes
                .iter()
                .find(|c| c.descriptor.id.eq_ignore_ascii_case(id) || c.descriptor.name == id)
                .ok_or_else(|| {
                    HostError::LoadError(format!("VST3 class '{}' not found in bundle", id))
                })?,
            None => &self.inner.classes[0],
        };

        let factory = self
            .inner
            .factory
            .as_ref()
            .ok_or_else(|| HostError::InitError("VST3 factory is missing".into()))?;

        unsafe {
            let mut comp_ptr: *mut Steinberg_Vst_IComponent = ptr::null_mut();
            let res = (factory.createInstance)(
                factory.as_void(),
                entry.cid.as_ptr() as Steinberg_FIDString,
                Steinberg_Vst_IComponent_iid.as_ptr() as Steinberg_FIDString,
                &mut comp_ptr as *mut _ as *mut *mut c_void,
            );
            let comp = ComPtr::from_raw(comp_ptr)
                .filter(|_| res == kResultOk)
                .ok_or_else(|| {
                    HostError::InitError(format!(
                        "factory->createInstance(IComponent) failed with tresult {}",
                        res
                    ))
                })?;

            let host_unknown =
                &*self.inner.host_app as *const Vst3HostApplication as *mut Steinberg_FUnknown;
            let init_res = (comp.initialize)(comp.as_void(), host_unknown);
            if init_res != kResultOk {
                return Err(HostError::InitError(format!(
                    "IComponent::initialize failed with tresult {}",
                    init_res
                )));
            }

            let Some(processor) = ComPtr::<Steinberg_Vst_IAudioProcessor>::query(comp.as_ptr())
            else {
                let _ = (comp.terminate)(comp.as_void());
                return Err(HostError::InitError(
                    "VST3 IComponent does not implement IAudioProcessor".into(),
                ));
            };

            let mut is_single_component = false;
            let controller = if let Some(ctrl) =
                ComPtr::<Steinberg_Vst_IEditController>::query(comp.as_ptr())
            {
                is_single_component = true;
                Some(ctrl)
            } else {
                let mut ctrl_cid: Steinberg_TUID = [0u8; 16];
                if (comp.getControllerClassId)(comp.as_void(), &mut ctrl_cid) == kResultOk
                    && ctrl_cid != [0u8; 16]
                {
                    let mut raw_ctrl: *mut Steinberg_Vst_IEditController = ptr::null_mut();
                    let c_res = (factory.createInstance)(
                        factory.as_void(),
                        ctrl_cid.as_ptr() as Steinberg_FIDString,
                        Steinberg_Vst_IEditController_iid.as_ptr() as Steinberg_FIDString,
                        &mut raw_ctrl as *mut _ as *mut *mut c_void,
                    );
                    ComPtr::from_raw(raw_ctrl)
                        .filter(|_| c_res == kResultOk)
                        .filter(|ctrl| (ctrl.initialize)(ctrl.as_void(), host_unknown) == kResultOk)
                } else {
                    None
                }
            };

            let shared_state = Arc::new(Mutex::new(SharedHostState::default()));
            let mut comp_handler = Vst3ComponentHandler::new_boxed(Arc::clone(&shared_state));
            let mut comp_cp = None;
            let mut ctrl_cp = None;

            if let Some(ref ctrl) = controller {
                let _ = (ctrl.setComponentHandler)(
                    ctrl.as_void(),
                    &mut *comp_handler as *mut Vst3ComponentHandler
                        as *mut Steinberg_Vst_IComponentHandler,
                );

                if !is_single_component {
                    if let (Some(c_cp), Some(k_cp)) = (
                        ComPtr::<Steinberg_Vst_IConnectionPoint>::query(comp.as_ptr()),
                        ComPtr::<Steinberg_Vst_IConnectionPoint>::query(ctrl.as_ptr()),
                    ) {
                        let _ = (c_cp.connect)(c_cp.as_void(), k_cp.as_ptr());
                        let _ = (k_cp.connect)(k_cp.as_void(), c_cp.as_ptr());
                        comp_cp = Some(c_cp);
                        ctrl_cp = Some(k_cp);
                    }

                    let mut stream = Vst3MemoryStream::new_empty();
                    if (comp.getState)(comp.as_void(), stream.as_ibstream_ptr()) == kResultOk
                        && !stream.data.is_empty()
                    {
                        stream.cursor = 0;
                        let _ = (ctrl.setComponentState)(ctrl.as_void(), stream.as_ibstream_ptr());
                    }
                }
            }

            activate_all_buses(&comp);

            let mut instance = Vst3PluginInstance::new(
                Arc::clone(&self.inner),
                entry.descriptor.clone(),
                comp,
                processor,
                controller,
                is_single_component,
                comp_cp,
                ctrl_cp,
                Vst3HostApplication::new_boxed(),
                comp_handler,
                Vst3PlugFrame::new_boxed(),
                shared_state,
            );
            instance.refresh_param_cache();
            Ok(Box::new(instance))
        }
    }
}

fn resolve_vst3_binary_path(path: &Path) -> HostResult<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let contents_dir = path.join("Contents");
    let arch_dirs = [
        "MacOS",
        "x86_64-win",
        "arm64x-win",
        "aarch64-win",
        "x86_64-linux",
        "aarch64-linux",
    ];
    for arch in arch_dirs {
        let bin_dir = contents_dir.join(arch);
        if bin_dir.is_dir() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                for cand in [
                    bin_dir.join(stem),
                    bin_dir.join(format!("{stem}.vst3")),
                    bin_dir.join(format!("{stem}.dll")),
                    bin_dir.join(format!("{stem}.so")),
                ] {
                    if cand.is_file() {
                        return Ok(cand);
                    }
                }
            }
            if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        return Ok(p);
                    }
                }
            }
        }
    }
    Err(HostError::LoadError(format!(
        "Could not locate binary inside VST3 bundle '{}'",
        path.display()
    )))
}

#[cfg(target_os = "macos")]
fn create_macos_cf_bundle(path: &Path) -> *mut c_void {
    use core_foundation::base::TCFType;
    use core_foundation::bundle::CFBundleCreate;
    use core_foundation::url::CFURL;

    let bundle_dir = if path.is_dir() {
        path.to_path_buf()
    } else if let Some(parent) = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
    {
        parent.to_path_buf()
    } else {
        return ptr::null_mut();
    };

    if let Some(url) = CFURL::from_path(&bundle_dir, true) {
        unsafe {
            CFBundleCreate(
                core_foundation::base::kCFAllocatorDefault,
                url.as_concrete_TypeRef(),
            ) as *mut c_void
        }
    } else {
        ptr::null_mut()
    }
}

pub(crate) unsafe fn enumerate_vst3_classes(
    factory: &ComPtr<Steinberg_IPluginFactory>,
) -> Vec<Vst3ClassEntry> {
    let mut entries = Vec::new();
    unsafe {
        let mut factory_info: Steinberg_PFactoryInfo = std::mem::zeroed();
        let _ = (factory.getFactoryInfo)(factory.as_void(), &mut factory_info);
        let default_vendor = cchar_slice_to_string(&factory_info.vendor);
        let url = cchar_slice_to_string(&factory_info.url);
        let email = cchar_slice_to_string(&factory_info.email);

        let factory2 = ComPtr::<Steinberg_IPluginFactory2>::query(factory.as_ptr());

        let count = (factory.countClasses)(factory.as_void());
        for i in 0..count {
            let mut info: Steinberg_PClassInfo = std::mem::zeroed();
            if (factory.getClassInfo)(factory.as_void(), i, &mut info) != kResultOk {
                continue;
            }
            let base_category = cchar_slice_to_string(&info.category);

            if let Some(ref f2) = factory2 {
                let mut info2: Steinberg_PClassInfo2 = std::mem::zeroed();
                if (f2.getClassInfo2)(f2.as_void(), i, &mut info2) == kResultOk {
                    let category2 = cchar_slice_to_string(&info2.category);
                    if base_category == kVstAudioEffectClass || category2 == kVstAudioEffectClass {
                        let name = cchar_slice_to_string(&info2.name);
                        let vendor_str = cchar_slice_to_string(&info2.vendor);
                        let vendor = if vendor_str.is_empty() {
                            default_vendor.clone()
                        } else {
                            vendor_str
                        };
                        let version = cchar_slice_to_string(&info2.version);
                        let mut sub_cats = cchar_slice_to_string(&info2.subCategories);
                        if sub_cats.is_empty() && category2 != kVstAudioEffectClass {
                            sub_cats = category2;
                        }
                        let features: Vec<String> = sub_cats
                            .split('|')
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect();
                        entries.push(Vst3ClassEntry {
                            cid: info2.cid,
                            descriptor: PluginDescriptorInfo {
                                format: PluginFormat::Vst3,
                                id: uid_to_hex(&info2.cid),
                                name: if name.is_empty() {
                                    cchar_slice_to_string(&info.name)
                                } else {
                                    name
                                },
                                vendor,
                                version,
                                url: url.clone(),
                                manual_url: String::new(),
                                support_url: email.clone(),
                                description: cchar_slice_to_string(&info2.sdkVersion),
                                features,
                            },
                        });
                    }
                    continue;
                }
            }

            if base_category == kVstAudioEffectClass {
                entries.push(Vst3ClassEntry {
                    cid: info.cid,
                    descriptor: PluginDescriptorInfo {
                        format: PluginFormat::Vst3,
                        id: uid_to_hex(&info.cid),
                        name: cchar_slice_to_string(&info.name),
                        vendor: default_vendor.clone(),
                        version: "1.0.0".into(),
                        url: url.clone(),
                        manual_url: String::new(),
                        support_url: email.clone(),
                        description: String::new(),
                        features: Vec::new(),
                    },
                });
            }
        }
    }
    entries
}

unsafe fn activate_all_buses(component: &ComPtr<Steinberg_Vst_IComponent>) {
    unsafe {
        for &media_type in &[
            Steinberg_Vst_MediaTypes_kAudio,
            Steinberg_Vst_MediaTypes_kEvent,
        ] {
            for &dir in &[
                Steinberg_Vst_BusDirections_kInput,
                Steinberg_Vst_BusDirections_kOutput,
            ] {
                let count = (component.getBusCount)(component.as_void(), media_type, dir);
                for idx in 0..count {
                    let mut info: Steinberg_Vst_BusInfo = std::mem::zeroed();
                    if (component.getBusInfo)(component.as_void(), media_type, dir, idx, &mut info)
                        == kResultOk
                    {
                        let should_activate = info.busType == Steinberg_Vst_BusTypes_kMain
                            || (info.flags & Steinberg_Vst_BusInfo_BusFlags_kDefaultActive) != 0;
                        if should_activate {
                            let _ = (component.activateBus)(
                                component.as_void(),
                                media_type,
                                dir,
                                idx,
                                1,
                            );
                        }
                    }
                }
            }
        }
    }
}
