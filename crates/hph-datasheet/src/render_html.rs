use crate::svg_plot::{
    render_svg_bar_chart, render_svg_linear_xy_chart, render_svg_log_freq_chart, xml_escape,
    BarItem, PlotTrace, PRINT_PALETTE,
};
use crate::{DatasheetFingerprint, SweepTraceSnapshot};
use hph_core::*;
use hph_dsp::*;
use hph_validate::*;
use std::fmt::Write;

#[allow(clippy::too_many_arguments)]
pub fn build_printable_html_datasheet(
    desc: &PluginDescriptorInfo,
    sample_rate: f64,
    audio_ports: &[AudioPortInfo],
    note_ports: &[NotePortInfo],
    params: &[(ParamInfo, f64, String)],
    latency_samples: Option<u32>,
    tail_samples: Option<u32>,
    gui_shot: Option<&(u32, u32, String)>,
    fp: &DatasheetFingerprint,
    val: &ValidationReport,
    bench: &BenchmarkReport,
    primary: &SweepTraceSnapshot,
    imd: &ImdReport,
    rt60: &Rt60Report,
    delay: &DelayReport,
    stereo: &StereoReport,
    sweep_traces: &[SweepTraceSnapshot],
    ref_snap: Option<&SweepTraceSnapshot>,
) -> String {
    let active_sources: Vec<&SweepTraceSnapshot> = if !sweep_traces.is_empty() {
        sweep_traces.iter().collect()
    } else {
        vec![primary]
    };

    // 1. Build Bode Magnitude, Phase, and Group Delay SVG charts
    let mut mag_traces = Vec::new();
    let mut phase_traces = Vec::new();
    let mut gd_traces = Vec::new();

    for (idx, src) in active_sources.iter().enumerate() {
        let color = PRINT_PALETTE[idx % PRINT_PALETTE.len()];
        mag_traces.push(PlotTrace {
            label: src.label.clone(),
            color,
            dashed: false,
            points: src
                .bode
                .points
                .iter()
                .map(|p| (p.freq_hz, p.magnitude_db))
                .collect(),
        });
        phase_traces.push(PlotTrace {
            label: src.label.clone(),
            color,
            dashed: false,
            points: src
                .bode
                .points
                .iter()
                .map(|p| (p.freq_hz, p.phase_deg))
                .collect(),
        });
        gd_traces.push(PlotTrace {
            label: src.label.clone(),
            color,
            dashed: false,
            points: src
                .bode
                .points
                .iter()
                .map(|p| (p.freq_hz, p.group_delay_ms))
                .collect(),
        });
    }
    if let Some(r) = ref_snap {
        mag_traces.push(PlotTrace {
            label: r.label.clone(),
            color: "#dc2626",
            dashed: true,
            points: r
                .bode
                .points
                .iter()
                .map(|p| (p.freq_hz, p.magnitude_db))
                .collect(),
        });
        phase_traces.push(PlotTrace {
            label: r.label.clone(),
            color: "#dc2626",
            dashed: true,
            points: r
                .bode
                .points
                .iter()
                .map(|p| (p.freq_hz, p.phase_deg))
                .collect(),
        });
    }

    let svg_bode_mag = render_svg_log_freq_chart(
        "Figure 1A — Frequency Magnitude Response (20 Hz – 20 kHz)",
        "dB",
        &mag_traces,
        None,
    );
    let svg_bode_phase = render_svg_log_freq_chart(
        "Figure 1B — Continuous Unwrapped Phase Response (deg)",
        "deg",
        &phase_traces,
        None,
    );
    let svg_bode_gd = render_svg_log_freq_chart(
        "Figure 1C — Group Delay vs. Frequency (ms)",
        "ms",
        &gd_traces,
        None,
    );

    // 2. Build Dynamics Transfer Curve & Gain Curve SVG charts
    let mut dyn_io_traces = Vec::new();
    let mut dyn_gain_traces = Vec::new();
    for (idx, src) in active_sources.iter().enumerate() {
        let color = PRINT_PALETTE[idx % PRINT_PALETTE.len()];
        dyn_io_traces.push(PlotTrace {
            label: src.label.clone(),
            color,
            dashed: false,
            points: src
                .dynamics
                .static_curve
                .iter()
                .map(|p| (p.input_dbfs, p.output_dbfs))
                .collect(),
        });
        dyn_gain_traces.push(PlotTrace {
            label: src.label.clone(),
            color,
            dashed: false,
            points: src
                .dynamics
                .static_curve
                .iter()
                .map(|p| (p.input_dbfs, p.gain_db))
                .collect(),
        });
    }
    if let Some(r) = ref_snap {
        dyn_io_traces.push(PlotTrace {
            label: r.label.clone(),
            color: "#dc2626",
            dashed: true,
            points: r
                .dynamics
                .static_curve
                .iter()
                .map(|p| (p.input_dbfs, p.output_dbfs))
                .collect(),
        });
    }

    let svg_dyn_io = render_svg_linear_xy_chart(
        "Figure 2A — Static Input-to-Output Transfer Curve (-48 dBFS to +24 dBFS)",
        "Input Level (dBFS @ 1 kHz)",
        "dBFS",
        &dyn_io_traces,
        (-48.0, 24.0),
        (-48.0, 24.0),
        true,
    );
    let svg_dyn_gain = render_svg_linear_xy_chart(
        "Figure 2B — Gain vs. Input Level (-48 dBFS to +24 dBFS)",
        "Input Level (dBFS @ 1 kHz)",
        "dB",
        &dyn_gain_traces,
        (-48.0, 24.0),
        (-30.0, 30.0),
        false,
    );

    // 3. Harmonic Distortion Bar Chart (H2..H10 + IMD spurs)
    let mut harm_bars = Vec::new();
    for h in &primary.thdn.harmonics {
        let height_above_floor = (h.level_dbc + 120.0).clamp(0.0, 120.0);
        let color = if h.order % 2 == 0 {
            "#1d4ed8" // Even harmonics (Blue)
        } else {
            "#dc2626" // Odd harmonics (Red)
        };
        harm_bars.push(BarItem {
            label: format!("H{} ({:.0}k)", h.order, h.freq_hz / 1000.0),
            value: height_above_floor,
            display_value: format!("{:.1} dBc", h.level_dbc),
            color,
        });
    }
    harm_bars.push(BarItem {
        label: "IMD2 (1k)".into(),
        value: (imd.diff_tone_dbc + 120.0).clamp(0.0, 120.0),
        display_value: format!("{:.1} dBc", imd.diff_tone_dbc),
        color: "#7c3aed",
    });
    harm_bars.push(BarItem {
        label: "IMD3- (18k)".into(),
        value: (imd.third_order_lower_dbc + 120.0).clamp(0.0, 120.0),
        display_value: format!("{:.1} dBc", imd.third_order_lower_dbc),
        color: "#d97706",
    });
    let svg_harmonics = render_svg_bar_chart(
        "Figure 3 — Harmonic Spurs (H2..H10: Blue=Even, Red=Odd) & Two-Tone IMD (Height above -120 dBc Floor)",
        "dB",
        &harm_bars,
        0.0,
        120.0,
    );

    // 4. Multi-Band Octave RT60 Decay Spectrum
    let mut rt60_bars = Vec::new();
    let max_rt60 = rt60
        .octave_bands
        .iter()
        .map(|b| b.rt60_sec)
        .fold(rt60.rt60_sec.max(0.5), f64::max)
        * 1.2;
    for b in &rt60.octave_bands {
        let lbl = if b.center_freq_hz >= 1000.0 {
            format!("{:.0} kHz", b.center_freq_hz / 1000.0)
        } else {
            format!("{:.0} Hz", b.center_freq_hz)
        };
        rt60_bars.push(BarItem {
            label: lbl,
            value: b.rt60_sec,
            display_value: format!("{:.2}s", b.rt60_sec),
            color: "#059669",
        });
    }
    let svg_rt60 = render_svg_bar_chart(
        "Figure 4 — ISO 3382 Reverberation Time (RT60) Across Octave Bands",
        "s",
        &rt60_bars,
        0.0,
        max_rt60,
    );

    // 5. CPU Load Bar Chart across buffer sizes
    let mut cpu_bars = Vec::new();
    let max_cpu = bench
        .dsp_configs
        .iter()
        .map(|c| c.cpu_load_percent)
        .fold(5.0f64, f64::max)
        * 1.25;
    for c in &bench.dsp_configs {
        cpu_bars.push(BarItem {
            label: format!("{} smp", c.block_size),
            value: c.cpu_load_percent,
            display_value: format!("{:.2}% ({:.0}x)", c.cpu_load_percent, c.realtime_speedup_x),
            color: "#0891b2",
        });
    }
    let svg_cpu = render_svg_bar_chart(
        "Figure 5 — Real-Time Audio Thread CPU Load (%) & Speedup Across Buffer Sizes",
        "%",
        &cpu_bars,
        0.0,
        max_cpu,
    );

    // Render printable white HTML document
    let mut html = String::with_capacity(32768);
    let _ = write!(
        html,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>{name} — Audio Plugin Engineering Datasheet</title>
<style>
  :root {{
    --bg: #ffffff;
    --fg: #0f172a;
    --muted: #475569;
    --border: #cbd5e1;
    --subtle: #f8fafc;
    --accent: #1d4ed8;
    --pass: #15803d;
    --pass-bg: #dcfce7;
    --fail: #b91c1c;
    --fail-bg: #fee2e2;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0;
    padding: 28px 36px;
    background: var(--bg);
    color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    font-size: 12px;
    line-height: 1.45;
  }}
  .sheet {{ max-width: 980px; margin: 0 auto; }}
  .header {{
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    border-bottom: 3px solid var(--fg);
    padding-bottom: 10px;
    margin-bottom: 16px;
  }}
  .header h1 {{ margin: 0; font-size: 22px; letter-spacing: -0.3px; }}
  .header .sub {{ color: var(--muted); font-size: 12px; margin-top: 3px; }}
  .header .doc-id {{ text-align: right; font-family: ui-monospace, SFMono-Regular, monospace; font-size: 11px; color: var(--muted); }}
  .badge {{
    display: inline-block;
    padding: 2px 7px;
    border-radius: 3px;
    font-weight: 700;
    font-size: 10px;
    text-transform: uppercase;
  }}
  .badge-pass {{ background: var(--pass-bg); color: var(--pass); border: 1px solid #86efac; }}
  .badge-fail {{ background: var(--fail-bg); color: var(--fail); border: 1px solid #fca5a5; }}
  .badge-info {{ background: #eff6ff; color: #1d4ed8; border: 1px solid #bfdbfe; margin-right: 5px; }}
  .section {{
    margin-bottom: 22px;
    page-break-inside: avoid;
  }}
  .section-title {{
    font-size: 13px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--fg);
    background: var(--subtle);
    border-left: 4px solid var(--accent);
    border-top: 1px solid var(--border);
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    padding: 6px 10px;
    margin: 0 0 10px 0;
  }}
  .grid-2 {{ display: grid; grid-template-columns: 1fr 1fr; gap: 14px; align-items: start; }}
  .kpi-row {{ display: grid; grid-template-columns: repeat(6, 1fr); gap: 8px; margin-bottom: 14px; }}
  .kpi {{
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 8px 10px;
    background: #ffffff;
  }}
  .kpi .k-lbl {{ font-size: 10px; color: var(--muted); text-transform: uppercase; }}
  .kpi .k-val {{ font-size: 14px; font-weight: 700; font-family: ui-monospace, SFMono-Regular, monospace; margin-top: 2px; }}
  table {{
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
    margin-bottom: 8px;
  }}
  th, td {{
    border: 1px solid var(--border);
    padding: 5px 8px;
    text-align: left;
  }}
  th {{ background: var(--subtle); font-weight: 600; color: var(--fg); }}
  td.mono {{ font-family: ui-monospace, SFMono-Regular, monospace; }}
  .spec-chart {{ width: 100%; height: auto; display: block; margin-bottom: 10px; page-break-inside: avoid; }}
  .gui-frame {{
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 8px;
    text-align: center;
    background: var(--subtle);
  }}
  .gui-frame img {{ max-width: 100%; max-height: 240px; border: 1px solid #94a3b8; }}
  @media print {{
    body {{ padding: 0; font-size: 10.5px; }}
    .section {{ page-break-inside: avoid; }}
  }}
</style>
</head>
<body>
<div class="sheet">
  <div class="header">
    <div>
      <h1>{name} <span style="font-weight:400;color:#64748b">v{ver}</span></h1>
      <div class="sub">Vendor: <strong>{vendor}</strong> | ID: <code>{id}</code> | Format: <strong>{fmt:?}</strong> | Features: {feats}</div>
    </div>
    <div class="doc-id">
      <div><strong>HPH ENGINEERING DATASHEET</strong></div>
      <div>Sample Rate: {sr:.0} Hz</div>
    </div>
  </div>
"#,
        name = xml_escape(&desc.name),
        ver = xml_escape(&desc.version),
        vendor = xml_escape(&desc.vendor),
        id = xml_escape(&desc.id),
        fmt = desc.format,
        feats = xml_escape(&desc.features.join(", ")),
        sr = sample_rate,
    );

    // Active Acoustic Fingerprint Banner & Key Metrics Row
    let _ = writeln!(html, r#"  <div style="margin-bottom:12px;">"#);
    let _ = writeln!(html, r#"    <strong>Acoustic Fingerprint:</strong> "#);
    if fp.frequency_shaping_active {
        let _ = writeln!(
            html,
            r#"<span class="badge badge-info">Linear EQ / Filter Shaping</span>"#
        );
    }
    if fp.nonlinear_harmonics_active {
        let _ = writeln!(
            html,
            r#"<span class="badge badge-info">Nonlinear Harmonics / Saturation</span>"#
        );
    }
    if fp.dynamic_compression_active {
        let _ = writeln!(
            html,
            r#"<span class="badge badge-info">Dynamic Compression / Limiting</span>"#
        );
    }
    if fp.spatial_tail_active {
        let _ = writeln!(
            html,
            r#"<span class="badge badge-info">Reverberant / Delay Tail</span>"#
        );
    }
    let _ = writeln!(html, r#"  </div>"#);

    let _ = writeln!(
        html,
        r#"  <div class="kpi-row">
    <div class="kpi"><div class="k-lbl">Validation</div><div class="k-val">{}/{} PASS</div></div>
    <div class="kpi"><div class="k-lbl">1 kHz Gain</div><div class="k-val">{:+.2} dB</div></div>
    <div class="kpi"><div class="k-lbl">THD+N @ 1kHz</div><div class="k-val">{:.4}%</div></div>
    <div class="kpi"><div class="k-lbl">Two-Tone IMD</div><div class="k-val">{:.4}%</div></div>
    <div class="kpi"><div class="k-lbl">Acoustic Delay</div><div class="k-val">{:.2} ms</div></div>
    <div class="kpi"><div class="k-lbl">Denormal Ratio</div><div class="k-val">{:.2}x</div></div>
  </div>"#,
        val.passed_checks,
        val.total_checks,
        primary.bode.gain_1khz_db,
        primary.thdn.thdn_percent,
        imd.imd_percent,
        delay.peak_delay_ms,
        bench.denormal_slowdown_ratio
    );

    // Section 1: Architecture, Ports, Parameters & GUI
    let _ = writeln!(
        html,
        r#"  <div class="section">
    <h2 class="section-title">1. Plugin Topology, Parameters &amp; Visual Interface</h2>
    <div class="grid-2">
      <div>
        <table>
          <thead><tr><th>Port</th><th>Direction</th><th>Channels</th><th>Type</th><th>PDC / Tail</th></tr></thead>
          <tbody>"#
    );
    for p in audio_ports {
        let _ = writeln!(
            html,
            r#"            <tr><td>{}</td><td class="mono">{}</td><td class="mono">{}</td><td>{}</td><td class="mono">Lat={:?} / Tail={:?}</td></tr>"#,
            xml_escape(&p.name),
            if p.is_input { "INPUT" } else { "OUTPUT" },
            p.channel_count,
            xml_escape(&p.port_type),
            latency_samples,
            tail_samples
        );
    }
    for np in note_ports {
        let _ = writeln!(
            html,
            r#"            <tr><td>{}</td><td class="mono">{}</td><td class="mono">MIDI/CLAP</td><td>Note Port</td><td class="mono">0x{:x}</td></tr>"#,
            xml_escape(&np.name),
            if np.is_input { "NOTE IN" } else { "NOTE OUT" },
            np.supported_dialects
        );
    }
    let _ = writeln!(
        html,
        r#"          </tbody>
        </table>
        <table>
          <thead><tr><th>ID</th><th>Parameter Name</th><th>Range</th><th>Default</th><th>Active Value</th></tr></thead>
          <tbody>"#
    );
    for (p, _cur, txt) in params.iter().take(16) {
        let _ = writeln!(
            html,
            r#"            <tr><td class="mono">{}</td><td>{}</td><td class="mono">[{:.2} .. {:.2}]</td><td class="mono">{:.2}</td><td class="mono">{}</td></tr>"#,
            p.id,
            xml_escape(&p.name),
            p.min_value,
            p.max_value,
            p.default_value,
            xml_escape(txt)
        );
    }
    let _ = writeln!(html, r#"          </tbody></table></div>"#);

    if let Some((w, h, uri)) = gui_shot {
        let _ = writeln!(
            html,
            r#"      <div class="gui-frame">
        <img src="{uri}" alt="Plugin GUI Screenshot"/>
        <div style="margin-top:6px;font-size:10px;color:#475569;">Offscreen Captured GUI ({w}&times;{h} px)</div>
      </div>"#
        );
    } else {
        let _ = writeln!(
            html,
            r#"      <div class="gui-frame" style="padding:36px 12px;color:#64748b;">
         Headless DSP Mode (No GUI Screenshot Captured)
      </div>"#
        );
    }
    let _ = writeln!(html, r#"    </div></div>"#);

    // Section 2: Linear Transfer Function (Bode Magnitude, Unwrapped Phase, Group Delay)
    let _ = writeln!(
        html,
        r#"  <div class="section">
    <h2 class="section-title">2. Linear Frequency Response, Phase Rotation &amp; Group Delay</h2>
    {svg_bode_mag}
    {svg_bode_phase}
    {svg_bode_gd}
  </div>"#
    );

    // Section 3: Dynamics, Compression & +24 dBFS Overdrive Transfer Curve
    let _ = writeln!(
        html,
        r#"  <div class="section">
    <h2 class="section-title">3. Dynamics Transfer Curve &amp; Headroom Overdrive (-48 dBFS to +24 dBFS)</h2>
    <div style="margin-bottom:8px;color:#334155;">
      <strong>Estimated Ratio:</strong> <code>{:.2}:1</code> &nbsp;|&nbsp;
      <strong>Threshold Knee:</strong> <code>{:?} dBFS</code> &nbsp;|&nbsp;
      <strong>Max Tone-Burst GR:</strong> <code>{:.2} dB</code> &nbsp;|&nbsp;
      <strong>Attack (10–90%):</strong> <code>{:.2} ms</code> &nbsp;|&nbsp;
      <strong>Release (10–90%):</strong> <code>{:.2} ms</code>
    </div>
    {svg_dyn_io}
    {svg_dyn_gain}
  </div>"#,
        primary.dynamics.estimated_ratio,
        primary.dynamics.estimated_threshold_dbfs,
        primary.dynamics.max_gain_reduction_db,
        primary.dynamics.attack_time_ms,
        primary.dynamics.release_time_ms
    );

    // Section 4: Harmonic Distortion (THD+N) & Two-Tone Intermodulation (IMD)
    let _ = writeln!(
        html,
        r#"  <div class="section">
    <h2 class="section-title">4. Nonlinear Harmonic Profile (THD+N) &amp; Two-Tone Intermodulation (IMD)</h2>
    <div style="margin-bottom:8px;color:#334155;">
      <strong>THD @ 1 kHz:</strong> <code>{:.4}% ({:.2} dB)</code> &nbsp;|&nbsp;
      <strong>Even Harmonics:</strong> <code>{:.2} dB</code> &nbsp;|&nbsp;
      <strong>Odd Harmonics:</strong> <code>{:.2} dB</code> &nbsp;|&nbsp;
      <strong>SINAD:</strong> <code>{:.2} dB</code> &nbsp;|&nbsp;
      <strong>Max Alias Spur:</strong> <code>{:.2} dBc</code> &nbsp;|&nbsp;
      <strong>IMD (19k+20k):</strong> <code>{:.4}% ({:.2} dB)</code>
    </div>
    {svg_harmonics}
  </div>"#,
        primary.thdn.thd_percent,
        primary.thdn.thd_db,
        primary.thdn.even_thd_db,
        primary.thdn.odd_thd_db,
        primary.thdn.sinad_db,
        primary.thdn.max_alias_spur_dbc,
        imd.imd_percent,
        imd.imd_db
    );

    // Section 5: Spatial Decay (9-Band Octave RT60), Echo Taps & Stereo Image
    let _ = writeln!(
        html,
        r#"  <div class="section">
    <h2 class="section-title">5. Spatial Decay (9-Band ISO Octave RT60), Acoustic Delay &amp; Stereo Image</h2>
    <div style="margin-bottom:8px;color:#334155;">
      <strong>Broadband RT60:</strong> <code>{:.3} s</code> (EDT={:.3}s, T20={:.3}s, T30={:.3}s, R&sup2;={:.3}) &nbsp;|&nbsp;
      <strong>Onset Delay:</strong> <code>{} smp ({:.2} ms)</code> &nbsp;|&nbsp;
      <strong>PDC Discrepancy:</strong> <code>{:?} smp</code> &nbsp;|&nbsp;
      <strong>Stereo Phase Correlation:</strong> <code>{:+.4}</code> (Width={:.3})
    </div>
    {svg_rt60}
  </div>"#,
        rt60.rt60_sec,
        rt60.edt_sec,
        rt60.t20_sec,
        rt60.t30_sec,
        rt60.r_squared,
        delay.onset_delay_samples,
        delay.onset_delay_ms,
        delay.latency_discrepancy_samples,
        stereo.phase_correlation,
        stereo.stereo_width
    );

    // Section 6: Real-Time CPU Performance & 9-Point Validation Scorecard
    let _ = writeln!(
        html,
        r#"  <div class="section">
    <h2 class="section-title">6. Real-Time Audio Performance &amp; Validation Scorecard</h2>
    {svg_cpu}
    <table>
      <thead><tr><th>Status</th><th>Category</th><th>Check Name</th><th>Duration</th><th>Telemetry &amp; Findings</th></tr></thead>
      <tbody>"#
    );
    for chk in &val.checks {
        let badge = if chk.passed {
            r#"<span class="badge badge-pass">PASS</span>"#
        } else {
            r#"<span class="badge badge-fail">FAIL</span>"#
        };
        let _ = writeln!(
            html,
            r#"        <tr><td>{badge}</td><td>{}</td><td><strong>{}</strong></td><td class="mono">{:.2} ms</td><td class="mono">{}</td></tr>"#,
            xml_escape(&chk.category),
            xml_escape(&chk.name),
            chk.duration_ms,
            xml_escape(&chk.details)
        );
    }
    let _ = writeln!(
        html,
        r#"      </tbody>
    </table>
  </div>
</div>
</body>
</html>"#
    );

    html
}
