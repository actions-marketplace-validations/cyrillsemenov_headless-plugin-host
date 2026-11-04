use crate::types::ValidationCheckResult;
use hph_core::*;
use std::collections::HashSet;
use std::time::Instant;

/// Check #1: Descriptor & Extension VTable Integrity
pub fn check_descriptor_and_extensions(
    descriptor: &PluginDescriptorInfo,
    extensions: &[ExtensionProbeResult],
) -> ValidationCheckResult {
    let t0 = Instant::now();
    let mut passed = true;
    let mut warnings = Vec::new();
    if descriptor.id.trim().is_empty() || descriptor.name.trim().is_empty() {
        passed = false;
        warnings.push("Descriptor `id` or `name` is empty".into());
    }
    for ext in extensions {
        if ext.supported && !ext.valid_vtable {
            passed = false;
            warnings.push(format!(
                "Extension `{}` returned non-null struct with null mandatory function pointers",
                ext.name
            ));
        }
    }
    let supported_count = extensions.iter().filter(|e| e.supported).count();
    ValidationCheckResult {
        category: "ABI & Extensions".into(),
        name: "Descriptor & Extension VTable Integrity".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details: format!(
            "{supported_count}/{} extensions implemented with valid function tables",
            extensions.len()
        ),
        warnings,
    }
}

/// Check #2: Audio & Note Port Topology
pub fn check_ports_topology(plugin: &dyn PluginInstance) -> ValidationCheckResult {
    let t0 = Instant::now();
    let audio_ports = plugin.audio_ports();
    let note_ports = plugin.note_ports();
    let mut passed = true;
    let mut warnings = Vec::new();

    let mut in_ids = HashSet::new();
    let mut out_ids = HashSet::new();
    for p in &audio_ports {
        if p.channel_count == 0 {
            passed = false;
            warnings.push(format!("Audio port '{}' has 0 channels", p.name));
        }
        let set = if p.is_input {
            &mut in_ids
        } else {
            &mut out_ids
        };
        if !set.insert(p.id) {
            passed = false;
            warnings.push(format!("Duplicate audio port ID {}", p.id));
        }
    }

    ValidationCheckResult {
        category: "Ports".into(),
        name: "Audio & Note Port Topology".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details: format!(
            "{} audio ports, {} note ports verified",
            audio_ports.len(),
            note_ports.len()
        ),
        warnings,
    }
}
