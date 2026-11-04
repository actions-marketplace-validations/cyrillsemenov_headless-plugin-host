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
use hph_core::PluginInstance;

/// Run the comprehensive validation suite on a plugin instance.
pub fn run_full_validation(plugin: &mut dyn PluginInstance, sample_rate: f64) -> ValidationReport {
    let descriptor = plugin.descriptor().clone();
    let extensions = plugin.probe_extensions();
    let mut checks = Vec::with_capacity(9);

    checks.push(check_descriptor_and_extensions(&descriptor, &extensions));
    checks.push(check_ports_topology(plugin));
    checks.push(check_parameters_contract(plugin));
    checks.push(check_state_and_reset_determinism(plugin, sample_rate));
    checks.push(check_block_size_invariance(plugin, sample_rate));
    checks.push(check_headroom_and_overload(plugin, sample_rate));

    let (rt_check, diagnostics) = check_realtime_allocation_and_threads(plugin, sample_rate);
    checks.push(rt_check);

    checks.push(check_bypass_and_zipper_noise(plugin, sample_rate));
    checks.push(check_extended_contracts(plugin));

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
