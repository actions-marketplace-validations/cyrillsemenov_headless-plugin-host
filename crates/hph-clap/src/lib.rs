pub mod bundle;
pub mod contracts;
pub mod events;
pub mod gui;
pub mod host;
pub mod instance;

pub use bundle::ClapBundle;
pub use instance::ClapPluginInstance;

#[cfg(test)]
mod tests {
    use super::*;
    use hph_core::{PluginBundle, PluginFormat};
    use std::path::Path;

    #[test]
    fn test_clap_bundle_trait_and_loader() {
        assert_eq!(ClapBundle::FORMAT, PluginFormat::Clap);
        assert_eq!(ClapBundle::EXTENSIONS, &[".clap"]);
        assert!(ClapBundle::matches_path(Path::new("/tmp/Test.clap")));
        assert!(ClapBundle::matches_path(Path::new(
            "/tmp/Test.clap/Contents/MacOS/Test"
        )));
        assert!(!ClapBundle::matches_path(Path::new("/tmp/Test.vst3")));

        assert_eq!(ClapBundle::LOADER.format, PluginFormat::Clap);
        assert!(ClapBundle::LOADER.matches_path(Path::new("/tmp/Test.clap")));
    }
}
