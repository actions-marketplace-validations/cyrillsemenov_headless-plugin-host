use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;

use crate::error::{HostError, HostResult};
use crate::types::{
    AudioPortInfo, AudioProcessBuffers, ExtensionProbeResult, HostDiagnostics, NotePortInfo,
    ParamInfo, PluginDescriptorInfo, PluginFormat, ProcessEvent, ProcessStatus,
};

/// Format-agnostic interface for plugin GUI lifecycle and main-thread pumping.
///
/// Serves as a supertrait of [`PluginInstance`] so format crates can implement
/// `impl PluginGui for ...` in a dedicated `gui.rs` module without duplicating
/// inherent `_impl` forwarding methods on the instance struct.
/// All methods provide sensible headless/no-GUI defaults.
pub trait PluginGui: Send {
    fn supports_gui(&self) -> bool {
        false
    }

    fn gui_create(&mut self) -> HostResult<()> {
        Err(HostError::Unsupported(
            "GUI is not supported by this plugin".into(),
        ))
    }

    fn gui_destroy(&mut self) {}

    fn gui_can_resize(&self) -> bool {
        false
    }

    fn gui_get_size(&self) -> Option<(u32, u32)> {
        None
    }

    fn gui_adjust_size(&self, _width: u32, _height: u32) -> Option<(u32, u32)> {
        None
    }

    fn gui_set_size(&mut self, _width: u32, _height: u32) -> HostResult<()> {
        Err(HostError::Unsupported(
            "GUI resize is not supported by this plugin".into(),
        ))
    }

    /// # Safety
    /// `parent_handle` must be a valid native window/view handle for the active OS
    /// (`NSView*` on macOS, `HWND` on Windows, X11 `Window` ID cast to `*mut c_void` on Linux).
    unsafe fn gui_set_parent(&mut self, _parent_handle: *mut c_void) -> HostResult<()> {
        Err(HostError::Unsupported(
            "GUI embedding is not supported by this plugin".into(),
        ))
    }

    fn gui_show(&mut self) -> HostResult<()> {
        Ok(())
    }

    fn gui_hide(&mut self) -> HostResult<()> {
        Ok(())
    }

    /// Pump any pending plugin main-thread callbacks or registered timers.
    fn tick_main_thread(&mut self, _elapsed_ms: u64) {}
}

/// Format-agnostic interface for probing supported plugin extensions and executing
/// format-specific deep contract checks.
///
/// Serves as a supertrait of [`PluginInstance`] so format crates can implement
/// `impl PluginContractChecks for ...` in a dedicated `contracts.rs` module without
/// duplicating inherent `_impl` forwarding methods on the instance struct.
pub trait PluginContractChecks: Send {
    /// Probe which format-specific extensions or interfaces the plugin implements.
    fn probe_extensions(&self) -> Vec<ExtensionProbeResult> {
        Vec::new()
    }

    /// Run format-specific deep extension and host contract checks (e.g., CLAP state-context,
    /// render mode, remote-controls, note-name, audio-ports-config, param-indication, context-menu;
    /// VST3 IMessage/IAttributeList, IUnitInfo, IMidiMapping; AUv2 FactoryPresets, BypassEffect).
    fn run_extended_contract_checks(&mut self) -> HostResult<Vec<String>> {
        Ok(Vec::new())
    }
}

/// Format-agnostic interface for a loaded audio plugin instance.
///
/// Combines [`PluginGui`] and [`PluginContractChecks`] as supertraits and provides
/// default implementations for optional lifecycle, parameter, state, latency, tail,
/// and diagnostic methods so implementations only need to specify what their format
/// actually customizes.
pub trait PluginInstance: PluginGui + PluginContractChecks + Send {
    fn descriptor(&self) -> &PluginDescriptorInfo;

    fn format(&self) -> PluginFormat {
        self.descriptor().format
    }

    fn audio_ports(&self) -> Vec<AudioPortInfo> {
        Vec::new()
    }

    fn note_ports(&self) -> Vec<NotePortInfo> {
        Vec::new()
    }

    fn params(&self) -> Vec<ParamInfo> {
        Vec::new()
    }

    fn get_param_value(&self, _param_id: u32) -> Option<f64> {
        None
    }

    fn param_value_to_text(&self, _param_id: u32, _value: f64) -> Option<String> {
        None
    }

    fn param_text_to_value(&self, _param_id: u32, _text: &str) -> Option<f64> {
        None
    }

    fn flush_params(
        &mut self,
        _events: &[ProcessEvent],
        _out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<()> {
        Ok(())
    }

    fn activate(&mut self, sample_rate: f64, min_frames: u32, max_frames: u32) -> HostResult<()>;
    fn deactivate(&mut self);

    fn start_processing(&mut self) -> HostResult<()> {
        Ok(())
    }

    fn stop_processing(&mut self) {}
    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffers: &mut AudioProcessBuffers<'_>,
        in_events: &[ProcessEvent],
        out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<ProcessStatus>;

    fn supports_state(&self) -> bool {
        false
    }

    fn save_state(&self) -> HostResult<Vec<u8>> {
        Err(HostError::StateError(
            "State saving is not supported by this plugin".into(),
        ))
    }

    fn load_state(&mut self, _data: &[u8]) -> HostResult<()> {
        Err(HostError::StateError(
            "State loading is not supported by this plugin".into(),
        ))
    }

    fn latency_samples(&self) -> Option<u32> {
        None
    }

    fn tail_samples(&self) -> Option<u32> {
        None
    }

    /// Retrieve and optionally clear host diagnostics (logs, thread-check violations).
    fn take_diagnostics(&mut self) -> HostDiagnostics {
        HostDiagnostics::default()
    }
}

fn path_matches_extensions(path: &Path, extensions: &[&str]) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    extensions.iter().any(|ext| {
        let slash = format!("{ext}/");
        let backslash = format!("{ext}\\");
        s.ends_with(ext) || s.contains(&slash) || s.contains(&backslash)
    })
}

/// Plain descriptor and constructor function for loading a [`PluginBundle`] from disk.
#[derive(Clone, Copy)]
pub struct FormatLoader {
    pub format: PluginFormat,
    pub extensions: &'static [&'static str],
    pub load: fn(&Path) -> HostResult<Arc<dyn PluginBundle>>,
}

impl FormatLoader {
    /// Construct a `FormatLoader` for a concrete [`PluginBundle`] type `B`.
    pub const fn new<B: PluginBundle + 'static>(
        format: PluginFormat,
        extensions: &'static [&'static str],
    ) -> Self {
        Self {
            format,
            extensions,
            load: |path| {
                let bundle = B::load(path)?;
                Ok(bundle)
            },
        }
    }

    pub fn matches_path(&self, path: &Path) -> bool {
        path_matches_extensions(path, self.extensions)
    }
}

/// Unified, `dyn`-compatible trait for a plugin bundle (`.clap`, `.vst3`, `.component`).
///
/// Scopes `where Self: Sized` to the static constructor and loader items (`loader`,
/// `load`, `matches_path`) so `Arc<dyn PluginBundle>` works directly without needing
/// a separate `DynPluginBundle` or `PhantomData` adapter trait hierarchy.
pub trait PluginBundle: Send + Sync {
    /// Return the static [`FormatLoader`] descriptor for this bundle format.
    fn loader() -> FormatLoader
    where
        Self: Sized;

    /// Load a plugin bundle from `path`.
    fn load(path: &Path) -> HostResult<Arc<Self>>
    where
        Self: Sized;

    /// Default path-matching helper using [`Self::loader()`].
    fn matches_path(path: &Path) -> bool
    where
        Self: Sized,
    {
        Self::loader().matches_path(path)
    }

    /// Return the plugin format of this bundle.
    fn format(&self) -> PluginFormat;

    /// Enumerate all plugin descriptors contained in this bundle.
    fn descriptors(&self) -> Vec<PluginDescriptorInfo>;

    /// Instantiate a boxed [`PluginInstance`] for the given plugin ID (or default first plugin).
    fn create_instance(&self, id: Option<&str>) -> HostResult<Box<dyn PluginInstance>>;
}

/// Load a plugin bundle from `path` using the provided [`FormatLoader`] slice.
/// First dispatches to the loader whose [`FormatLoader::matches_path`] returns `true`;
/// if no extension matches (e.g., a raw `.dylib`/`.so`/`.dll` path), probes each loader in order.
pub fn load_bundle_with_loaders(
    path: &Path,
    loaders: &[FormatLoader],
) -> HostResult<Arc<dyn PluginBundle>> {
    for loader in loaders {
        if loader.matches_path(path) {
            return (loader.load)(path);
        }
    }
    let mut last_err = None;
    for loader in loaders {
        match (loader.load)(path) {
            Ok(bundle) => return Ok(bundle),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        HostError::LoadError(format!(
            "Failed to load '{}' with any registered plugin format loader",
            path.display()
        ))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyInstance {
        desc: PluginDescriptorInfo,
    }

    impl PluginGui for DummyInstance {}
    impl PluginContractChecks for DummyInstance {}

    impl PluginInstance for DummyInstance {
        fn descriptor(&self) -> &PluginDescriptorInfo {
            &self.desc
        }

        fn activate(
            &mut self,
            _sample_rate: f64,
            _min_frames: u32,
            _max_frames: u32,
        ) -> HostResult<()> {
            Ok(())
        }

        fn deactivate(&mut self) {}

        fn process(
            &mut self,
            _buffers: &mut AudioProcessBuffers<'_>,
            _in_events: &[ProcessEvent],
            _out_events: &mut Vec<ProcessEvent>,
        ) -> HostResult<ProcessStatus> {
            Ok(ProcessStatus::Continue)
        }
    }

    struct DummyBundle {
        desc: PluginDescriptorInfo,
    }

    impl DummyBundle {
        pub const LOADER: FormatLoader = FormatLoader::new::<Self>(PluginFormat::Clap, &[".clap"]);
    }

    impl PluginBundle for DummyBundle {
        fn loader() -> FormatLoader {
            Self::LOADER
        }

        fn load(path: &Path) -> HostResult<Arc<Self>> {
            if path.to_string_lossy().contains("fail") {
                return Err(HostError::LoadError("Simulated load failure".into()));
            }
            Ok(Arc::new(Self {
                desc: PluginDescriptorInfo {
                    format: Self::LOADER.format,
                    id: "com.test.dummy".into(),
                    name: "Dummy".into(),
                    vendor: "Test".into(),
                    version: "1.0".into(),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description: String::new(),
                    features: vec![],
                },
            }))
        }

        fn format(&self) -> PluginFormat {
            Self::LOADER.format
        }

        fn descriptors(&self) -> Vec<PluginDescriptorInfo> {
            vec![self.desc.clone()]
        }

        fn create_instance(&self, _id: Option<&str>) -> HostResult<Box<dyn PluginInstance>> {
            Ok(Box::new(DummyInstance {
                desc: self.desc.clone(),
            }))
        }
    }

    #[test]
    fn test_plugin_bundle_trait_and_format_loader() {
        let loader = DummyBundle::LOADER;
        assert_eq!(loader.format, PluginFormat::Clap);
        assert!(loader.matches_path(Path::new("/tmp/MySynth.clap")));
        assert!(loader.matches_path(Path::new("/tmp/MySynth.CLAP/Contents/MacOS/MySynth")));
        assert!(loader.matches_path(Path::new(
            r"C:\Plugins\MySynth.clap\Contents\x86_64-win\MySynth.clap"
        )));
        assert!(!loader.matches_path(Path::new("/tmp/MySynth.vst3")));

        let loaders = &[loader];
        let dyn_bundle = load_bundle_with_loaders(Path::new("/tmp/MySynth.clap"), loaders)
            .expect("Expected bundle to load");
        assert_eq!(dyn_bundle.format(), PluginFormat::Clap);
        assert_eq!(dyn_bundle.descriptors().len(), 1);

        let mut inst = dyn_bundle
            .create_instance(None)
            .expect("Expected boxed instance");
        assert_eq!(inst.descriptor().id, "com.test.dummy");
        assert_eq!(inst.format(), PluginFormat::Clap);
        assert!(!inst.supports_gui());
        assert!(!inst.supports_state());
        assert!(inst.probe_extensions().is_empty());
        assert!(inst.run_extended_contract_checks().unwrap().is_empty());

        // Fallback when extension doesn't match (.dylib)
        let fallback_bundle = load_bundle_with_loaders(Path::new("/tmp/libdummy.dylib"), loaders)
            .expect("Expected fallback load to succeed");
        assert_eq!(fallback_bundle.format(), PluginFormat::Clap);

        // Empty loader slice returns LoadError
        let empty_err = load_bundle_with_loaders(Path::new("/tmp/MySynth.clap"), &[]);
        assert!(empty_err.is_err());
    }
}
