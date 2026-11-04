use crate::cli::{load_plugin_bundle, InspectOutput};
use anyhow::Result;
use hph_dsp::linear_to_db;
use hph_gui::capture_plugin_screenshot;
use hph_validate::{run_benchmarks, run_full_validation, run_fuzz_torture, FuzzConfig};
use std::path::Path;

pub fn run_inspect(plugin: &Path, id: Option<&str>, json_output: bool) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let _ = instance.activate(48000.0, 1, 1024);

    let out = InspectOutput {
        bundle_descriptors: bundle.descriptors(),
        active_plugin: instance.descriptor().clone(),
        extensions: instance.probe_extensions(),
        audio_ports: instance.audio_ports(),
        note_ports: instance.note_ports(),
        parameters: instance.params(),
        latency_samples: instance.latency_samples(),
        tail_samples: instance.tail_samples(),
    };
    instance.deactivate();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("=== Plugin Descriptor ===");
        println!("  Format:      {:?}", out.active_plugin.format);
        println!("  Name:        {}", out.active_plugin.name);
        println!("  ID:          {}", out.active_plugin.id);
        println!("  Vendor:      {}", out.active_plugin.vendor);
        println!("  Version:     {}", out.active_plugin.version);
        println!("  Features:    {}", out.active_plugin.features.join(", "));
        println!("  Latency:     {:?} samples", out.latency_samples);
        println!("  Tail:        {:?} samples", out.tail_samples);

        println!("\n=== Supported Extensions ===");
        for ext in out.extensions.iter().filter(|e| e.supported) {
            let status = if ext.valid_vtable {
                "OK"
            } else {
                "INVALID VTABLE"
            };
            if ext.notes.is_empty() {
                println!("  [{}] {}", status, ext.name);
            } else {
                println!("  [{}] {} ({})", status, ext.name, ext.notes);
            }
        }

        println!("\n=== Audio Ports ({}) ===", out.audio_ports.len());
        for p in &out.audio_ports {
            println!(
                "  [{}] id={} '{}' ({} ch, type='{}', main={})",
                if p.is_input { "IN " } else { "OUT" },
                p.id,
                p.name,
                p.channel_count,
                p.port_type,
                p.is_main
            );
        }

        println!("\n=== Note Ports ({}) ===", out.note_ports.len());
        for p in &out.note_ports {
            println!(
                "  [{}] id={} '{}' (dialects=0x{:x})",
                if p.is_input { "IN " } else { "OUT" },
                p.id,
                p.name,
                p.supported_dialects
            );
        }

        println!("\n=== Parameters ({}) ===", out.parameters.len());
        for p in &out.parameters {
            let cur = instance.get_param_value(p.id).unwrap_or(p.default_value);
            let txt = instance.param_value_to_text(p.id, cur).unwrap_or_default();
            println!(
                "  id={:<6} {:<24} [{:.2} .. {:.2}] def={:.2} cur={:.2} ('{}') module='{}'",
                p.id, p.name, p.min_value, p.max_value, p.default_value, cur, txt, p.module
            );
        }
    }
    Ok(())
}

pub fn run_validate(
    plugin: &Path,
    id: Option<&str>,
    sample_rate: f64,
    json_output: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let report = run_full_validation(&mut *instance, sample_rate);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "=== Validation Report: {} ({}) ===",
            report.descriptor.name, report.descriptor.id
        );
        for check in &report.checks {
            let badge = if check.passed { "PASS" } else { "FAIL" };
            println!(
                "  [{}] {:<18} | {:<45} ({:.2} ms)",
                badge, check.category, check.name, check.duration_ms
            );
            println!("         -> {}", check.details);
            for w in &check.warnings {
                println!("         WARN: {w}");
            }
        }
        println!(
            "\nSummary: {}/{} checks passed ({} failed)",
            report.passed_checks, report.total_checks, report.failed_checks
        );
    }
    if report.failed_checks > 0 {
        std::process::exit(1);
    }
    Ok(())
}

pub fn run_bench(
    plugin: &Path,
    id: Option<&str>,
    blocks: usize,
    gui: bool,
    json_output: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let report = run_benchmarks(
        &mut *instance,
        &[44100.0, 48000.0, 96000.0],
        &[64, 128, 256, 512, 1024],
        blocks,
        gui,
    )?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("=== DSP Benchmark: {} ===", report.descriptor.name);
        println!(
            "  {:<8} {:<7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>10} {:>9}",
            "SR (Hz)",
            "Block",
            "Mean µs",
            "P50 µs",
            "P99 µs",
            "Max µs",
            "CPU Load",
            "Speedup",
            "RT Allocs"
        );
        for c in &report.dsp_configs {
            println!(
                "  {:<8.0} {:<7} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>9.2}% {:>9.1}x {:>9}",
                c.sample_rate,
                c.block_size,
                c.mean_us,
                c.p50_us,
                c.p99_us,
                c.max_us,
                c.cpu_load_percent,
                c.realtime_speedup_x,
                c.rt_allocs.alloc_calls
            );
        }
        println!(
            "\n  Denormal CPU Slowdown Ratio: {:.2}x (1.0x = immune to subnormal stalls)",
            report.denormal_slowdown_ratio
        );
        if let Some(ref g) = report.gui_benchmark {
            println!("\n=== GUI Benchmark ===");
            println!("  Supported:          {}", g.supported);
            println!(
                "  Initial Size:       {}x{}",
                g.initial_size.0, g.initial_size.1
            );
            println!("  Create Latency:     {:.1} µs", g.create_latency_us);
            println!("  SetParent Latency:  {:.1} µs", g.set_parent_latency_us);
            println!("  Show Latency:       {:.1} µs", g.show_latency_us);
            println!(
                "  Frame Pump (60Hz):  avg {:.1} µs / max {:.1} µs",
                g.avg_frame_pump_us, g.max_frame_pump_us
            );
            if let Some(rz) = g.resize_avg_latency_us {
                println!("  Resize Latency:     {:.1} µs", rz);
            }
            println!("  RGBA Capture Time:  {:.1} µs", g.capture_rgba_latency_us);
        }
    }
    Ok(())
}

pub fn run_fuzz(
    plugin: &Path,
    id: Option<&str>,
    iterations: usize,
    seed: u64,
    max_block_size: usize,
    corrupt_state: bool,
    json_output: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let config = FuzzConfig {
        seed,
        iterations,
        max_block_size,
        corrupt_state,
    };
    let report = run_fuzz_torture(&mut *instance, &config);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let badge = if report.passed { "PASS" } else { "FAIL" };
        println!(
            "=== Fuzz Torture Report [{}]: {} ({}) ===",
            badge, report.descriptor.name, report.descriptor.id
        );
        println!(
            "  Seed:              0x{:016X} ({})",
            report.seed, report.seed
        );
        println!(
            "  Iterations:        {} ({} frames, {} events, {} state mutations) in {:.1} ms",
            report.iterations_completed,
            report.total_frames_processed,
            report.total_events_dispatched,
            report.state_mutations_tested,
            report.duration_ms
        );
        println!(
            "  Numerical Health:  Max |y|={:.4} ({:.2} dBFS), NaN/Inf={}, Runaway(>80dBFS)={}",
            report.max_abs_sample,
            linear_to_db(report.max_abs_sample),
            report.nan_inf_samples,
            report.runaway_samples
        );
        println!(
            "  RT Allocations:    {} allocs, {} frees ({} bytes)",
            report.rt_allocs.alloc_calls,
            report.rt_allocs.free_calls,
            report.rt_allocs.bytes_allocated
        );
        for f in &report.failures {
            println!(
                "  FAILURE [iter #{} @ {:.0} Hz, blk={}]: {}",
                f.iteration, f.sample_rate, f.block_size, f.reason
            );
        }
    }
    if !report.passed {
        std::process::exit(1);
    }
    Ok(())
}

pub fn run_screenshot(
    plugin: &Path,
    id: Option<&str>,
    output: &Path,
    warmup_ms: u64,
    width: Option<u32>,
    height: Option<u32>,
    json_output: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let custom_size = match (width, height) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    let res = capture_plugin_screenshot(&mut *instance, output, warmup_ms, custom_size)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&res)?);
    } else {
        println!(
            "Captured GUI screenshot ({}x{}, {} non-blank pixels) -> {}",
            res.width, res.height, res.non_blank_pixels, res.output_path
        );
        println!(
            "  Timings: create={:.1} µs, show={:.1} µs, warmup={} ms, capture={:.1} µs",
            res.create_time_us, res.show_time_us, res.warmup_ms, res.capture_time_us
        );
    }
    Ok(())
}
