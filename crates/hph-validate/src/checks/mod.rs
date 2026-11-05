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
use hph_core::{ParamInfo, PluginInstance, ProcessEvent};

pub(crate) fn is_safe_restore_param(p: &ParamInfo) -> bool {
    if p.flags.is_readonly || p.flags.is_hidden || (p.flags.raw_bits & (1 << 15)) != 0 {
        return false;
    }
    let name_lc = p.name.to_ascii_lowercase();
    !name_lc.contains("preset")
        && !name_lc.contains("program")
        && !name_lc.contains("midi cc")
        && !name_lc.contains("trigger")
}

pub(crate) fn is_safe_fuzz_param(p: &ParamInfo) -> bool {
    if !is_safe_restore_param(p) || p.flags.is_bypass {
        return false;
    }
    let name_lc = p.name.to_ascii_lowercase();
    !name_lc.contains("bypass")
        && !name_lc.contains("enable")
        && !name_lc.contains("panic")
        && !name_lc.contains("reset")
}

struct InitialPluginSnapshot {
    state: Option<Vec<u8>>,
    param_events: Vec<ProcessEvent>,
}

impl InitialPluginSnapshot {
    fn capture(plugin: &mut dyn PluginInstance) -> Self {
        let state = if plugin.supports_state() {
            plugin.save_state().ok()
        } else {
            None
        };
        let param_events = plugin
            .params()
            .into_iter()
            .filter(is_safe_restore_param)
            .map(|p| {
                let val = plugin.get_param_value(p.id).unwrap_or(p.default_value);
                ProcessEvent::ParamValue {
                    sample_offset: 0,
                    param_id: p.id,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: val,
                }
            })
            .collect();
        Self {
            state,
            param_events,
        }
    }

    fn restore(&self, plugin: &mut dyn PluginInstance) {
        if let Some(ref bytes) = self.state {
            let _ = plugin.load_state(bytes);
        }
        if !self.param_events.is_empty() {
            let mut out = Vec::new();
            let _ = plugin.flush_params(&self.param_events, &mut out);
        }
    }
}

/// Run the comprehensive validation suite on a plugin instance.
pub fn run_full_validation(plugin: &mut dyn PluginInstance, sample_rate: f64) -> ValidationReport {
    let initial = InitialPluginSnapshot::capture(plugin);
    let descriptor = plugin.descriptor().clone();
    let extensions = plugin.probe_extensions();
    let mut checks = Vec::with_capacity(9);

    checks.push(check_descriptor_and_extensions(&descriptor, &extensions));
    checks.push(check_ports_topology(plugin));
    checks.push(check_parameters_contract(plugin));
    initial.restore(plugin);
    checks.push(check_state_and_reset_determinism(plugin, sample_rate));
    initial.restore(plugin);
    checks.push(check_block_size_invariance(plugin, sample_rate));
    checks.push(check_headroom_and_overload(plugin, sample_rate));

    let (rt_check, diagnostics) = check_realtime_allocation_and_threads(plugin, sample_rate);
    checks.push(rt_check);

    checks.push(check_bypass_and_zipper_noise(plugin, sample_rate));
    initial.restore(plugin);
    checks.push(check_extended_contracts(plugin));
    initial.restore(plugin);

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
