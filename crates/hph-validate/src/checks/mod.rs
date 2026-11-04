pub mod abi_and_ports;
pub mod params_and_state;
pub mod rt_and_invariance;
pub mod signal_and_contracts;

pub use abi_and_ports::{check_descriptor_and_extensions, check_ports_topology};
pub use params_and_state::{check_parameters_contract, check_state_and_reset_determinism};
pub use rt_and_invariance::{check_block_size_invariance, check_realtime_allocation_and_threads};
pub use signal_and_contracts::{
    check_bypass_and_zipper_noise, check_extended_contracts, check_headroom_and_overload,
};

use crate::types::ValidationReport;
use hph_core::{PluginInstance, ProcessEvent};

fn restore_default_parameters(plugin: &mut dyn PluginInstance) {
    let default_events: Vec<ProcessEvent> = plugin
        .params()
        .into_iter()
        .filter(|p| !p.flags.is_readonly)
        .map(|p| ProcessEvent::ParamValue {
            sample_offset: 0,
            param_id: p.id,
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value: p.default_value,
        })
        .collect();
    if !default_events.is_empty() {
        let mut out = Vec::new();
        let _ = plugin.flush_params(&default_events, &mut out);
    }
}

/// Run the comprehensive validation suite on a plugin instance.
pub fn run_full_validation(plugin: &mut dyn PluginInstance, sample_rate: f64) -> ValidationReport {
    let descriptor = plugin.descriptor().clone();
    let extensions = plugin.probe_extensions();
    let mut checks = Vec::with_capacity(9);

    checks.push(check_descriptor_and_extensions(&descriptor, &extensions));
    checks.push(check_ports_topology(plugin));
    checks.push(check_parameters_contract(plugin));
    restore_default_parameters(plugin);
    checks.push(check_state_and_reset_determinism(plugin, sample_rate));
    restore_default_parameters(plugin);
    checks.push(check_block_size_invariance(plugin, sample_rate));
    checks.push(check_headroom_and_overload(plugin, sample_rate));

    let (rt_check, diagnostics) = check_realtime_allocation_and_threads(plugin, sample_rate);
    checks.push(rt_check);

    checks.push(check_bypass_and_zipper_noise(plugin, sample_rate));
    restore_default_parameters(plugin);
    checks.push(check_extended_contracts(plugin));
    restore_default_parameters(plugin);

    let total_checks = checks.len();
    let passed_checks = checks.iter().filter(|c| c.passed).count();
    let failed_checks = total_checks - passed_checks;

    ValidationReport {
        descriptor,
        total_checks,
        passed_checks,
        failed_checks,
        checks,
        extensions,
        diagnostics,
    }
}
