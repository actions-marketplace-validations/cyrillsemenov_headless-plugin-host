use hph_clap::ClapBundle;
use hph_core::*;
use hph_dsp::*;
use hph_validate::*;
use std::path::PathBuf;

fn mock_plugin_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    let filename = "libhph_mock_plugin.dylib";
    #[cfg(target_os = "linux")]
    let filename = "libhph_mock_plugin.so";
    #[cfg(target_os = "windows")]
    let filename = "hph_mock_plugin.dll";

    // First check alongside the current test executable (handles cross-compiled target/<triple>/debug/)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(deps_dir) = exe.parent() {
            if let Some(profile_dir) = deps_dir.parent() {
                let candidate = profile_dir.join(filename);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir.join("../../target/debug");
    target_dir.join(filename)
}

#[test]
fn integration_full_pipeline_on_mock_clap_plugin() {
    let dylib = mock_plugin_path();
    if !dylib.exists() {
        // Build mock plugin if not already present
        let status = std::process::Command::new("cargo")
            .args(["build", "-p", "hph-mock-plugin"])
            .status()
            .expect("Failed to invoke cargo build");
        assert!(status.success());
    }

    let bundle = load_bundle_with_loaders(&dylib, &[ClapBundle::LOADER])
        .expect("Should load mock CLAP dylib");
    assert_eq!(bundle.format(), PluginFormat::Clap);
    assert_eq!(bundle.descriptors().len(), 1);
    assert_eq!(bundle.descriptors()[0].id, "com.hph.mock-filter-saturator");

    let mut plugin = bundle
        .create_instance(None)
        .expect("Should instantiate mock plugin");

    // 1. Run full validation suite
    let val_report = run_full_validation(&mut *plugin, 48000.0);
    assert_eq!(
        val_report.failed_checks, 0,
        "Expected 0 failed validation checks, got: {:#?}",
        val_report.checks
    );
    assert_eq!(val_report.passed_checks, 9);
    assert!(
        val_report.checks[5].details.contains("Hot(+24dB) Gain=")
            && val_report.checks[5].details.contains("Recovery delta="),
        "Expected Check #6 to report +24 dBFS overload & recovery telemetry, got: {}",
        val_report.checks[5].details
    );
    assert!(
        val_report.checks[8]
            .details
            .contains("clap.state-context (3 contexts)"),
        "Expected Check #9 to verify clap.state-context, got: {}",
        val_report.checks[8].details
    );
    assert!(
        val_report.checks[8]
            .details
            .contains("clap.context-menu (1 items)"),
        "Expected Check #9 to verify clap.context-menu, got: {}",
        val_report.checks[8].details
    );

    // 2. Render saturated 1 kHz sine and check 4x True-Peak & THD+N
    let sr = 48000.0;
    let n = 48000;
    let sine = generate_signal(
        &SignalSpec::Sine {
            freq_hz: 1000.0,
            amplitude: 0.7,
            phase_rad: 0.0,
        },
        sr,
        n,
    );
    let in_bufs = vec![sine.clone(), sine];
    let drive_ev = [ProcessEvent::ParamValue {
        sample_offset: 0,
        param_id: 102, // Saturation Drive
        note_id: -1,
        port_index: -1,
        channel: -1,
        key: -1,
        value: 0.75,
    }];

    plugin.activate(sr, 1, 512).unwrap();
    plugin.start_processing().unwrap();
    plugin.reset();
    let (out_bufs, _, rt_stats) =
        process_audio_stream(&mut *plugin, &in_bufs, 2, n, 256, &drive_ev).unwrap();
    plugin.stop_processing();
    plugin.deactivate();

    assert_eq!(rt_stats.alloc_calls, 0);
    let meter = analyze_buffer_4x(&out_bufs[0]);
    assert!(meter.true_peak_4x_dbtp > -6.0 && meter.true_peak_4x_dbtp < 1.0);
    assert_eq!(meter.nan_count, 0);

    let thdn = analyze_thdn(&out_bufs[0][4096..], sr, 1000.0);
    assert!(
        thdn.thd_percent > 5.0,
        "Expected saturation drive to produce >5% THD, got {}",
        thdn.thd_percent
    );

    // 3. Benchmark DSP (and Win32/X11 GUI when on Windows or Linux with DISPLAY; macOS AppKit requires main thread)
    let include_gui = cfg!(target_os = "windows")
        || (cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_some());
    let bench = run_benchmarks(&mut *plugin, &[48000.0], &[128, 256], 25, include_gui).unwrap();
    assert_eq!(bench.dsp_configs.len(), 2);
    assert!(bench.dsp_configs[0].realtime_speedup_x > 10.0);
    if include_gui {
        let gui_bench = bench.gui_benchmark.expect("Expected GUI benchmark report");
        assert!(gui_bench.supported);
        assert_eq!(gui_bench.initial_size, (480, 300));
    }

    // 4. Run seeded Fuzz Torture Suite (including corrupted state loads)
    let fuzz_rep = run_fuzz_torture(
        &mut *plugin,
        &FuzzConfig {
            seed: 0x1234_5678_9ABC_DEF0,
            iterations: 100,
            max_block_size: 512,
            corrupt_state: true,
        },
    );
    assert!(
        fuzz_rep.passed,
        "Expected fuzz torture suite to pass on mock plugin, failures: {:#?}",
        fuzz_rep.failures
    );
    assert_eq!(fuzz_rep.nan_inf_samples, 0);
    assert_eq!(fuzz_rep.runaway_samples, 0);
    assert!(fuzz_rep.state_mutations_tested > 0);
}
