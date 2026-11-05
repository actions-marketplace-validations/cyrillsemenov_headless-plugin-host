use crate::events::ClapInputEventQueue;
use crate::host::ClapHostContext;
use crate::instance::ClapPluginInstance;
use hph_clap_sys::*;
use hph_core::*;
use libloading::Library;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Resolve a `.clap` path (which may be a macOS bundle directory `Foo.clap/Contents/MacOS/Foo`,
/// a Windows/Linux directory containing `.clap`/`.dll`/`.so`, or a direct binary file) to the
/// actual dynamic library file path and the bundle path.
pub fn resolve_clap_paths(input_path: &Path) -> HostResult<(PathBuf, PathBuf)> {
    if !input_path.exists() {
        return Err(HostError::LoadError(format!(
            "Plugin path does not exist: {}",
            input_path.display()
        )));
    }

    if input_path.is_file() {
        return Ok((input_path.to_path_buf(), input_path.to_path_buf()));
    }

    // macOS bundle directory: <Name>.clap/Contents/MacOS/<Executable>
    let macos_dir = input_path.join("Contents").join("MacOS");
    if macos_dir.is_dir() {
        if let Some(stem) = input_path.file_stem().and_then(|s| s.to_str()) {
            let candidate = macos_dir.join(stem);
            if candidate.is_file() {
                return Ok((candidate, input_path.to_path_buf()));
            }
        }
        if let Ok(entries) = std::fs::read_dir(&macos_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    return Ok((p, input_path.to_path_buf()));
                }
            }
        }
    }

    // Windows / Linux bundle directory fallback: look for .clap / .dll / .so inside the folder
    if let Ok(entries) = std::fs::read_dir(input_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("clap")
                        || ext.eq_ignore_ascii_case("dll")
                        || ext.eq_ignore_ascii_case("so")
                    {
                        return Ok((p, input_path.to_path_buf()));
                    }
                }
            }
        }
    }

    Err(HostError::LoadError(format!(
        "Could not locate binary inside CLAP bundle: {}",
        input_path.display()
    )))
}

pub(crate) unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

pub(crate) unsafe fn fixed_cstr_to_string(buf: &[c_char]) -> String {
    let bytes: &[u8] = std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len());
    let nul_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..nul_pos]).into_owned()
}

unsafe fn parse_descriptor(desc: *const clap_plugin_descriptor_t) -> PluginDescriptorInfo {
    let d = &*desc;
    let mut features = Vec::new();
    if !d.features.is_null() {
        let mut idx = 0;
        loop {
            let f_ptr = *d.features.add(idx);
            if f_ptr.is_null() {
                break;
            }
            features.push(cstr_to_string(f_ptr));
            idx += 1;
            if idx > 128 {
                break;
            }
        }
    }
    PluginDescriptorInfo {
        format: PluginFormat::Clap,
        id: cstr_to_string(d.id),
        name: cstr_to_string(d.name),
        vendor: cstr_to_string(d.vendor),
        version: cstr_to_string(d.version),
        url: cstr_to_string(d.url),
        manual_url: cstr_to_string(d.manual_url),
        support_url: cstr_to_string(d.support_url),
        description: cstr_to_string(d.description),
        features,
    }
}

pub(crate) struct ClapBundleInner {
    _lib: Library,
    entry: *const clap_plugin_entry_t,
    pub(crate) factory: *const clap_plugin_factory_t,
    pub(crate) descriptors: Vec<PluginDescriptorInfo>,
}

unsafe impl Send for ClapBundleInner {}
unsafe impl Sync for ClapBundleInner {}

impl Drop for ClapBundleInner {
    fn drop(&mut self) {
        unsafe {
            if !self.entry.is_null() {
                if let Some(deinit) = (*self.entry).deinit {
                    deinit();
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ClapBundle {
    pub(crate) inner: Arc<ClapBundleInner>,
}

impl ClapBundle {
    pub const FORMAT: PluginFormat = PluginFormat::Clap;
    pub const EXTENSIONS: &'static [&'static str] = &[".clap"];
    pub const LOADER: FormatLoader = FormatLoader::new::<Self>(Self::FORMAT, Self::EXTENSIONS);
}

impl PluginBundle for ClapBundle {
    fn loader() -> FormatLoader {
        Self::LOADER
    }

    fn load(path: &Path) -> HostResult<Arc<Self>> {
        let (bin_path, bundle_path) = resolve_clap_paths(path)?;
        unsafe {
            let lib = Library::new(&bin_path).map_err(|e| {
                HostError::LoadError(format!("dlopen failed for {}: {e}", bin_path.display()))
            })?;

            let entry_sym: libloading::Symbol<*const clap_plugin_entry_t> = lib
                .get(b"clap_entry\0")
                .map_err(|e| HostError::LoadError(format!("Missing `clap_entry` symbol: {e}")))?;

            let entry_ptr = *entry_sym;
            if entry_ptr.is_null() {
                return Err(HostError::LoadError("`clap_entry` pointer is null".into()));
            }

            let entry = &*entry_ptr;
            if !clap_version_is_compatible(entry.clap_version) {
                return Err(HostError::LoadError(format!(
                    "Incompatible CLAP version: {}.{}.{}",
                    entry.clap_version.major, entry.clap_version.minor, entry.clap_version.revision
                )));
            }

            let init_fn = entry.init.ok_or_else(|| {
                HostError::InitError("`clap_entry.init` function pointer is null".into())
            })?;

            let bundle_cstr = CString::new(bundle_path.to_string_lossy().as_bytes())
                .map_err(|e| HostError::InitError(format!("Invalid path CString: {e}")))?;

            if !init_fn(bundle_cstr.as_ptr()) {
                return Err(HostError::InitError(
                    "`clap_entry.init()` returned false".into(),
                ));
            }

            let get_factory = entry
                .get_factory
                .ok_or_else(|| HostError::InitError("`clap_entry.get_factory` is null".into()))?;

            let factory_ptr = get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr() as *const c_char)
                as *const clap_plugin_factory_t;
            if factory_ptr.is_null() {
                if let Some(deinit) = entry.deinit {
                    deinit();
                }
                return Err(HostError::InitError(
                    "`get_factory(\"clap.plugin-factory\")` returned null".into(),
                ));
            }

            let factory = &*factory_ptr;
            let count_fn = factory
                .get_plugin_count
                .ok_or_else(|| HostError::InitError("`factory.get_plugin_count` is null".into()))?;
            let desc_fn = factory.get_plugin_descriptor.ok_or_else(|| {
                HostError::InitError("`factory.get_plugin_descriptor` is null".into())
            })?;

            let count = count_fn(factory_ptr);
            let mut descriptors = Vec::with_capacity(count as usize);
            for i in 0..count {
                let d_ptr = desc_fn(factory_ptr, i);
                if !d_ptr.is_null() {
                    descriptors.push(parse_descriptor(d_ptr));
                }
            }

            Ok(Arc::new(Self {
                inner: Arc::new(ClapBundleInner {
                    _lib: lib,
                    entry: entry_ptr,
                    factory: factory_ptr,
                    descriptors,
                }),
            }))
        }
    }

    fn format(&self) -> PluginFormat {
        Self::FORMAT
    }

    fn descriptors(&self) -> Vec<PluginDescriptorInfo> {
        self.inner.descriptors.clone()
    }

    fn create_instance(&self, plugin_id: Option<&str>) -> HostResult<Box<dyn PluginInstance>> {
        let target_desc =
            if let Some(req_id) = plugin_id {
                self.inner
                    .descriptors
                    .iter()
                    .find(|d| d.id == req_id || d.name.eq_ignore_ascii_case(req_id))
                    .cloned()
                    .ok_or_else(|| {
                        HostError::InitError(format!("Plugin ID '{req_id}' not found in bundle"))
                    })?
            } else {
                self.inner.descriptors.first().cloned().ok_or_else(|| {
                    HostError::InitError("No plugins found in CLAP factory".into())
                })?
            };

        let host_ctx = ClapHostContext::new_boxed();
        let id_cstr = CString::new(target_desc.id.as_bytes()).unwrap();

        unsafe {
            let create_fn = (*self.inner.factory)
                .create_plugin
                .ok_or_else(|| HostError::InitError("`factory.create_plugin` is null".into()))?;

            let plugin_ptr = create_fn(self.inner.factory, &host_ctx.host_struct, id_cstr.as_ptr());
            if plugin_ptr.is_null() {
                return Err(HostError::InitError(format!(
                    "`create_plugin` returned null for '{}'",
                    target_desc.id
                )));
            }
            host_ctx.plugin_ptr.store(
                plugin_ptr as *mut clap_plugin_t,
                std::sync::atomic::Ordering::Release,
            );

            let init_fn = (*plugin_ptr).init.ok_or_else(|| {
                HostError::InitError("`plugin.init` function pointer is null".into())
            })?;

            if !init_fn(plugin_ptr) {
                if let Some(destroy) = (*plugin_ptr).destroy {
                    destroy(plugin_ptr);
                }
                return Err(HostError::InitError(format!(
                    "`plugin.init()` returned false for '{}'",
                    target_desc.id
                )));
            }

            let mut instance = ClapPluginInstance {
                _bundle: Arc::clone(&self.inner),
                host_ctx,
                plugin: plugin_ptr,
                descriptor: target_desc,
                is_active: false,
                is_processing: false,
                gui_created: false,
                param_cookies: HashMap::new(),
                in_port_channel_counts: Vec::new(),
                out_port_channel_counts: Vec::new(),
                scratch_in_ptrs: Vec::with_capacity(16),
                scratch_out_ptrs: Vec::with_capacity(16),
                scratch_dummy_in: Vec::new(),
                scratch_dummy_out: Vec::new(),
                scratch_in_bufs: Vec::with_capacity(8),
                scratch_out_bufs: Vec::with_capacity(8),
                scratch_in_events: ClapInputEventQueue::with_capacity(256),
            };
            instance.refresh_param_cookies();
            Ok(Box::new(instance))
        }
    }
}
