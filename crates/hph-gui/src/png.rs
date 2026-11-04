use hph_core::{HostError, HostResult};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub fn save_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> HostResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let file = File::create(path).map_err(|e| {
        HostError::GuiError(format!("Failed to create PNG {}: {e}", path.display()))
    })?;
    let w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| HostError::GuiError(format!("PNG header error: {e}")))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| HostError::GuiError(format!("PNG write error: {e}")))?;
    Ok(())
}
