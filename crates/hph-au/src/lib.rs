#![cfg(target_os = "macos")]

pub mod bundle;
pub mod contracts;
pub mod gui;
pub mod instance;
pub mod render_context;

pub use bundle::AuBundle;
pub use instance::AuPluginInstance;

#[cfg(test)]
mod tests {
    use super::*;
    use hph_core::{PluginBundle, PluginFormat};
    use std::path::Path;

    #[test]
    fn test_au_bundle_trait_and_loader() {
        assert_eq!(AuBundle::FORMAT, PluginFormat::AuV2);
        assert_eq!(AuBundle::EXTENSIONS, &[".component"]);
        assert!(AuBundle::matches_path(Path::new("/tmp/Test.component")));
        assert!(AuBundle::matches_path(Path::new(
            "/tmp/Test.component/Contents/MacOS/Test"
        )));
        assert!(!AuBundle::matches_path(Path::new("/tmp/Test.clap")));

        assert_eq!(AuBundle::LOADER.format, PluginFormat::AuV2);
        assert!(AuBundle::LOADER.matches_path(Path::new("/tmp/Test.component")));
    }
}
