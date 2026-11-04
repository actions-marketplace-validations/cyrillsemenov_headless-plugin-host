use std::fmt::Write;

pub const PRINT_PALETTE: &[&str] = &[
    "#1d4ed8", // Cobalt Blue
    "#dc2626", // Crimson Red
    "#059669", // Emerald Green
    "#d97706", // Amber Gold
    "#7c3aed", // Royal Violet
    "#0891b2", // Deep Cyan
];

#[derive(Debug, Clone)]
pub struct PlotTrace {
    pub label: String,
    pub color: &'static str,
    pub dashed: bool,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct BarItem {
    pub label: String,
    pub value: f64,
    pub display_value: String,
    pub color: &'static str,
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct LegendPlacement {
    x: f64,
    y: f64,
}

fn compute_legend_layout(
    traces: &[PlotTrace],
    pad_l: f64,
    max_x: f64,
) -> (usize, Vec<LegendPlacement>) {
    if traces.len() <= 1 {
        return (0, Vec::new());
    }
    let col_w = traces
        .iter()
        .map(|tr| (tr.label.chars().count() as f64) * 5.8 + 30.0)
        .fold(100.0_f64, f64::max);
    let avail_w = (max_x - pad_l).max(col_w);
    let cols = ((avail_w / col_w).floor() as usize).clamp(1, traces.len());
    let rows = traces.len().div_ceil(cols);

    let mut placements = Vec::with_capacity(traces.len());
    for idx in 0..traces.len() {
        let col = idx % cols;
        let row = idx / cols;
        let x = pad_l + (col as f64) * col_w;
        let y = 36.0 + (row as f64) * 15.0;
        placements.push(LegendPlacement { x, y });
    }
    (rows, placements)
}

fn write_svg_legend(
    svg: &mut String,
    traces: &[PlotTrace],
    placements: &[LegendPlacement],
    width: f64,
    pad_r: f64,
) {
    if traces.len() > 1 {
        for (tr, pos) in traces.iter().zip(placements.iter()) {
            let label_esc = xml_escape(&tr.label);
            let dash = if tr.dashed {
                r#" stroke-dasharray="4,3""#
            } else {
                ""
            };
            let _ = writeln!(
                svg,
                r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="2.2"{dash}/>"##,
                pos.x,
                pos.y,
                pos.x + 15.0,
                pos.y,
                tr.color
            );
            let _ = writeln!(
                svg,
                r##"  <text x="{:.1}" y="{:.1}" font-family="system-ui, -apple-system, sans-serif" font-size="9.5" fill="#334155">{label_esc}</text>"##,
                pos.x + 19.0,
                pos.y + 3.5
            );
        }
    } else if let Some(tr) = traces.first() {
        let label_esc = xml_escape(&tr.label);
        let leg_x = width - pad_r - ((tr.label.chars().count() as f64) * 6.0 + 28.0);
        let _ = writeln!(
            svg,
            r##"  <line x1="{:.1}" y1="18" x2="{:.1}" y2="18" stroke="{}" stroke-width="2.2"/>"##,
            leg_x,
            leg_x + 18.0,
            tr.color
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{:.1}" y="22" font-family="system-ui, -apple-system, sans-serif" font-size="10" fill="#334155">{label_esc}</text>"##,
            leg_x + 22.0
        );
    }
}

fn compute_nice_y_axis(min_v: f64, max_v: f64, y_unit: &str) -> (f64, f64, Vec<f64>) {
    if !min_v.is_finite() || !max_v.is_finite() {
        return (-24.0, 24.0, vec![-24.0, -12.0, 0.0, 12.0, 24.0]);
    }
    let min_v = if y_unit == "dB" {
        min_v.max(-60.0)
    } else if y_unit == "deg" {
        min_v.max(-360.0)
    } else {
        min_v.max(-20.0)
    };
    let max_v = if y_unit == "dB" {
        max_v.min(36.0).max(min_v + 3.0)
    } else if y_unit == "deg" {
        max_v.min(360.0).max(min_v + 15.0)
    } else {
        max_v.min(50.0).max(min_v + 0.5)
    };
    let span_raw = (max_v - min_v).max(1.0);
    let pad = span_raw * 0.08;
    let lo_target = if min_v >= -1e-6 && y_unit == "ms" {
        0.0
    } else {
        min_v - pad
    };
    let hi_target = max_v + pad;
    let candidates: &[f64] = if y_unit == "deg" {
        &[5.0, 10.0, 15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 180.0]
    } else if y_unit == "ms" {
        &[0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0]
    } else {
        &[1.0, 2.0, 3.0, 5.0, 6.0, 10.0, 12.0, 15.0, 20.0, 30.0]
    };

    let mut chosen_step = candidates[candidates.len() - 1];
    for &step in candidates {
        let lo = (lo_target / step).floor() * step;
        let hi = (hi_target / step).ceil() * step;
        let count = ((hi - lo) / step).round() as usize;
        if (4..=7).contains(&count) {
            chosen_step = step;
            break;
        }
    }
    while ((hi_target - lo_target) / chosen_step) > 8.0 {
        chosen_step *= 2.0;
    }

    let mut y_min = (lo_target / chosen_step).floor() * chosen_step;
    let mut y_max = (hi_target / chosen_step).ceil() * chosen_step;
    if (y_max - y_min) < chosen_step * 3.0 {
        if min_v >= -1e-6 && y_unit == "ms" {
            y_min = 0.0;
            y_max = chosen_step * 4.0;
        } else {
            y_min -= chosen_step;
            y_max += chosen_step;
        }
    }
    let n_steps = ((y_max - y_min) / chosen_step).round().max(1.0) as usize;
    let mut ticks = Vec::with_capacity(n_steps + 1);
    for i in 0..=n_steps {
        let v = y_min + (i as f64) * chosen_step;
        ticks.push(if v.abs() < 1e-9 { 0.0 } else { v });
    }
    (y_min, y_max, ticks)
}

/// Render a vector SVG logarithmic-frequency chart (`20 Hz .. 20 kHz`) suitable for print & PDF.
pub fn render_svg_log_freq_chart(
    title: &str,
    y_unit: &str,
    traces: &[PlotTrace],
    y_range_override: Option<(f64, f64)>,
) -> String {
    let width = 760.0f64;
    let pad_l = 62.0f64;
    let pad_r = 24.0f64;
    let (legend_rows, placements) = compute_legend_layout(traces, pad_l, width - pad_r);
    let extra_h = (legend_rows as f64) * 15.0;
    let height = 260.0f64 + extra_h;
    let pad_t = 36.0f64 + extra_h;
    let pad_b = 38.0f64;
    let plot_w = width - pad_l - pad_r;
    let plot_h = height - pad_t - pad_b;

    let f_min = 20.0f64;
    let f_max = 20000.0f64;
    let log_min = f_min.log10();
    let log_max = f_max.log10();

    let (y_min, y_max, y_ticks) = if let Some((ymin, ymax)) = y_range_override {
        compute_nice_y_axis(ymin, ymax, y_unit)
    } else {
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;
        for tr in traces {
            for &(_, y) in &tr.points {
                if y.is_finite() {
                    min_v = min_v.min(y);
                    max_v = max_v.max(y);
                }
            }
        }
        compute_nice_y_axis(min_v, max_v, y_unit)
    };
    let y_span = (y_max - y_min).max(1e-6);

    let map_x = |f: f64| -> f64 {
        let fc = f.clamp(f_min, f_max);
        pad_l + ((fc.log10() - log_min) / (log_max - log_min)) * plot_w
    };
    let map_y = |y: f64| -> f64 {
        let yc = y.clamp(y_min, y_max);
        pad_t + (1.0 - (yc - y_min) / y_span) * plot_h
    };

    let mut svg = String::with_capacity(4096);
    let _ = writeln!(
        svg,
        r##"<svg class="spec-chart" viewBox="0 0 {width:.0} {height:.0}" xmlns="http://www.w3.org/2000/svg">"##
    );
    let _ = writeln!(
        svg,
        r##"  <rect x="0" y="0" width="{width:.0}" height="{height:.0}" fill="#ffffff" stroke="#cbd5e1" stroke-width="1" rx="4"/>"##
    );
    let _ = writeln!(
        svg,
        r##"  <text x="{pad_l:.0}" y="22" font-family="system-ui, -apple-system, sans-serif" font-size="12" font-weight="700" fill="#0f172a">{}</text>"##,
        xml_escape(title)
    );

    write_svg_legend(&mut svg, traces, &placements, width, pad_r);

    // Minor vertical logarithmic decade gridlines (30..90, 300..900, 3k..9k)
    let minor_freqs: &[f64] = &[
        30.0, 40.0, 60.0, 70.0, 80.0, 90.0, 300.0, 400.0, 600.0, 700.0, 800.0, 900.0, 3000.0,
        4000.0, 6000.0, 7000.0, 8000.0, 9000.0,
    ];
    for &f in minor_freqs {
        let px = map_x(f);
        let _ = writeln!(
            svg,
            r##"  <line x1="{px:.1}" y1="{pad_t:.1}" x2="{px:.1}" y2="{:.1}" stroke="#f1f5f9" stroke-width="1"/>"##,
            pad_t + plot_h
        );
    }

    // Major vertical log-frequency gridlines & labels
    let major_freqs: &[(f64, &str)] = &[
        (20.0, "20 Hz"),
        (50.0, "50"),
        (100.0, "100 Hz"),
        (200.0, "200"),
        (500.0, "500"),
        (1000.0, "1 kHz"),
        (2000.0, "2k"),
        (5000.0, "5k"),
        (10000.0, "10 kHz"),
        (20000.0, "20 kHz"),
    ];
    for &(f, lbl) in major_freqs {
        let px = map_x(f);
        let is_decade =
            (f - 100.0).abs() < 1.0 || (f - 1000.0).abs() < 1.0 || (f - 10000.0).abs() < 1.0;
        let stroke = if is_decade { "#cbd5e1" } else { "#e2e8f0" };
        let _ = writeln!(
            svg,
            r##"  <line x1="{px:.1}" y1="{pad_t:.1}" x2="{px:.1}" y2="{:.1}" stroke="{stroke}" stroke-width="1"/>"##,
            pad_t + plot_h
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{px:.1}" y="{:.1}" text-anchor="middle" font-family="ui-monospace, SFMono-Regular, monospace" font-size="9.5" fill="#475569">{lbl}</text>"##,
            pad_t + plot_h + 15.0
        );
    }

    // Horizontal Y gridlines at clean snapped intervals
    let decimals = if y_ticks.iter().any(|v| (v.fract()).abs() > 1e-3) {
        1usize
    } else {
        0usize
    };
    for &val in &y_ticks {
        let py = map_y(val);
        let is_zero = val.abs() < 1e-6;
        if is_zero {
            let _ = writeln!(
                svg,
                r##"  <line x1="{pad_l:.1}" y1="{py:.1}" x2="{:.1}" y2="{py:.1}" stroke="#94a3b8" stroke-width="1.2" stroke-dasharray="4,3"/>"##,
                pad_l + plot_w
            );
        } else {
            let _ = writeln!(
                svg,
                r##"  <line x1="{pad_l:.1}" y1="{py:.1}" x2="{:.1}" y2="{py:.1}" stroke="#e2e8f0" stroke-width="1"/>"##,
                pad_l + plot_w
            );
        }
        let val_str = if is_zero {
            format!("0.{:0>width$}", "", width = decimals)
                .trim_end_matches('.')
                .to_string()
        } else if y_unit == "ms" && val >= 0.0 {
            format!("{val:.decimals$}")
        } else {
            format!("{val:+.decimals$}")
        };
        let _ = writeln!(
            svg,
            r##"  <text x="{:.1}" y="{:.1}" text-anchor="end" font-family="ui-monospace, SFMono-Regular, monospace" font-size="9.5" fill="#475569">{val_str} {y_unit}</text>"##,
            pad_l - 6.0,
            py + 3.0
        );
    }

    // Plot border
    let _ = writeln!(
        svg,
        r##"  <rect x="{pad_l:.1}" y="{pad_t:.1}" width="{plot_w:.1}" height="{plot_h:.1}" fill="none" stroke="#94a3b8" stroke-width="1"/>"##
    );

    // Render traces
    for tr in traces {
        if tr.points.is_empty() {
            continue;
        }
        let mut pts_str = String::new();
        for &(f, y) in &tr.points {
            if f.is_finite() && y.is_finite() {
                let _ = write!(pts_str, "{:.1},{:.1} ", map_x(f), map_y(y));
            }
        }
        let dash = if tr.dashed {
            r#" stroke-dasharray="5,3""#
        } else {
            ""
        };
        let _ = writeln!(
            svg,
            r##"  <polyline fill="none" stroke="{}" stroke-width="2.0" stroke-linejoin="round" stroke-linecap="round"{dash} points="{}"/>"##,
            tr.color,
            pts_str.trim()
        );
    }

    let _ = writeln!(svg, "</svg>");
    svg
}

/// Render a vector SVG linear X/Y chart (e.g. Dynamics Transfer Curve `-48 .. +24 dBFS`).
pub fn render_svg_linear_xy_chart(
    title: &str,
    x_label: &str,
    y_unit: &str,
    traces: &[PlotTrace],
    x_range: (f64, f64),
    y_range: (f64, f64),
    draw_unity_diagonal: bool,
) -> String {
    let width = 760.0f64;
    let pad_l = 62.0f64;
    let pad_r = 24.0f64;
    let (legend_rows, placements) = compute_legend_layout(traces, pad_l, width - pad_r);
    let extra_h = (legend_rows as f64) * 15.0;
    let height = 250.0f64 + extra_h;
    let pad_t = 36.0f64 + extra_h;
    let pad_b = 38.0f64;
    let plot_w = width - pad_l - pad_r;
    let plot_h = height - pad_t - pad_b;

    let (x_min, x_max) = x_range;
    let (y_min, y_max) = y_range;
    let x_span = (x_max - x_min).max(1e-6);
    let y_span = (y_max - y_min).max(1e-6);

    let map_x = |x: f64| -> f64 { pad_l + ((x.clamp(x_min, x_max) - x_min) / x_span) * plot_w };
    let map_y =
        |y: f64| -> f64 { pad_t + (1.0 - (y.clamp(y_min, y_max) - y_min) / y_span) * plot_h };

    let mut svg = String::with_capacity(4096);
    let _ = writeln!(
        svg,
        r##"<svg class="spec-chart" viewBox="0 0 {width:.0} {height:.0}" xmlns="http://www.w3.org/2000/svg">"##
    );
    let _ = writeln!(
        svg,
        r##"  <rect x="0" y="0" width="{width:.0}" height="{height:.0}" fill="#ffffff" stroke="#cbd5e1" stroke-width="1" rx="4"/>"##
    );
    let _ = writeln!(
        svg,
        r##"  <text x="{pad_l:.0}" y="22" font-family="system-ui, -apple-system, sans-serif" font-size="12" font-weight="700" fill="#0f172a">{}</text>"##,
        xml_escape(title)
    );

    write_svg_legend(&mut svg, traces, &placements, width, pad_r);

    // Y gridlines
    let y_steps = 6usize;
    for i in 0..=y_steps {
        let val = y_min + (i as f64 / y_steps as f64) * y_span;
        let py = map_y(val);
        let _ = writeln!(
            svg,
            r##"  <line x1="{pad_l:.1}" y1="{py:.1}" x2="{:.1}" y2="{py:.1}" stroke="#e2e8f0" stroke-width="1"/>"##,
            pad_l + plot_w
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{:.1}" y="{:.1}" text-anchor="end" font-family="ui-monospace, SFMono-Regular, monospace" font-size="9.5" fill="#475569">{val:+.0} {y_unit}</text>"##,
            pad_l - 6.0,
            py + 3.0
        );
    }

    // X gridlines
    let x_steps = 6usize;
    for i in 0..=x_steps {
        let val = x_min + (i as f64 / x_steps as f64) * x_span;
        let px = map_x(val);
        let _ = writeln!(
            svg,
            r##"  <line x1="{px:.1}" y1="{pad_t:.1}" x2="{px:.1}" y2="{:.1}" stroke="#e2e8f0" stroke-width="1"/>"##,
            pad_t + plot_h
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{px:.1}" y="{:.1}" text-anchor="middle" font-family="ui-monospace, SFMono-Regular, monospace" font-size="9.5" fill="#475569">{val:+.0} dB</text>"##,
            pad_t + plot_h + 15.0
        );
    }

    if draw_unity_diagonal {
        let p1x = map_x(x_min.max(y_min));
        let p1y = map_y(x_min.max(y_min));
        let p2x = map_x(x_max.min(y_max));
        let p2y = map_y(x_max.min(y_max));
        let _ = writeln!(
            svg,
            r##"  <line x1="{p1x:.1}" y1="{p1y:.1}" x2="{p2x:.1}" y2="{p2y:.1}" stroke="#94a3b8" stroke-width="1.2" stroke-dasharray="4,4"/>"##
        );
    }

    let _ = writeln!(
        svg,
        r##"  <rect x="{pad_l:.1}" y="{pad_t:.1}" width="{plot_w:.1}" height="{plot_h:.1}" fill="none" stroke="#94a3b8" stroke-width="1"/>"##
    );
    let _ = writeln!(
        svg,
        r##"  <text x="{:.1}" y="{:.1}" text-anchor="middle" font-family="system-ui, -apple-system, sans-serif" font-size="10" fill="#64748b">{}</text>"##,
        pad_l + plot_w * 0.5,
        height - 6.0,
        xml_escape(x_label)
    );

    for tr in traces {
        if tr.points.is_empty() {
            continue;
        }
        let mut pts_str = String::new();
        for &(x, y) in &tr.points {
            if x.is_finite() && y.is_finite() {
                let _ = write!(pts_str, "{:.1},{:.1} ", map_x(x), map_y(y));
            }
        }
        let dash = if tr.dashed {
            r#" stroke-dasharray="5,3""#
        } else {
            ""
        };
        let _ = writeln!(
            svg,
            r##"  <polyline fill="none" stroke="{}" stroke-width="2.0" stroke-linejoin="round" stroke-linecap="round"{dash} points="{}"/>"##,
            tr.color,
            pts_str.trim()
        );
        for &(x, y) in &tr.points {
            if x.is_finite() && y.is_finite() {
                let _ = writeln!(
                    svg,
                    r##"  <circle cx="{:.1}" cy="{:.1}" r="2.5" fill="{}"/>"##,
                    map_x(x),
                    map_y(y),
                    tr.color
                );
            }
        }
    }

    let _ = writeln!(svg, "</svg>");
    svg
}

/// Render a vector SVG vertical bar/stem chart (Harmonics H2..H10, 9-band Octave RT60, CPU Load).
pub fn render_svg_bar_chart(
    title: &str,
    y_unit: &str,
    bars: &[BarItem],
    y_min: f64,
    y_max: f64,
) -> String {
    let width = 760.0f64;
    let height = 230.0f64;
    let pad_l = 62.0f64;
    let pad_r = 24.0f64;
    let pad_t = 36.0f64;
    let pad_b = 34.0f64;
    let plot_w = width - pad_l - pad_r;
    let plot_h = height - pad_t - pad_b;
    let y_span = (y_max - y_min).max(1e-6);

    let map_y =
        |y: f64| -> f64 { pad_t + (1.0 - (y.clamp(y_min, y_max) - y_min) / y_span) * plot_h };

    let mut svg = String::with_capacity(4096);
    let _ = writeln!(
        svg,
        r##"<svg class="spec-chart" viewBox="0 0 {width:.0} {height:.0}" xmlns="http://www.w3.org/2000/svg">"##
    );
    let _ = writeln!(
        svg,
        r##"  <rect x="0" y="0" width="{width:.0}" height="{height:.0}" fill="#ffffff" stroke="#cbd5e1" stroke-width="1" rx="4"/>"##
    );
    let _ = writeln!(
        svg,
        r##"  <text x="{pad_l:.0}" y="22" font-family="system-ui, -apple-system, sans-serif" font-size="12" font-weight="700" fill="#0f172a">{}</text>"##,
        xml_escape(title)
    );

    let y_steps = 4usize;
    for i in 0..=y_steps {
        let val = y_min + (i as f64 / y_steps as f64) * y_span;
        let py = map_y(val);
        let _ = writeln!(
            svg,
            r##"  <line x1="{pad_l:.1}" y1="{py:.1}" x2="{:.1}" y2="{py:.1}" stroke="#e2e8f0" stroke-width="1"/>"##,
            pad_l + plot_w
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{:.1}" y="{:.1}" text-anchor="end" font-family="ui-monospace, SFMono-Regular, monospace" font-size="9.5" fill="#475569">{val:.1} {y_unit}</text>"##,
            pad_l - 6.0,
            py + 3.0
        );
    }

    let _ = writeln!(
        svg,
        r##"  <rect x="{pad_l:.1}" y="{pad_t:.1}" width="{plot_w:.1}" height="{plot_h:.1}" fill="none" stroke="#94a3b8" stroke-width="1"/>"##
    );

    let n = bars.len().max(1) as f64;
    let slot_w = plot_w / n;
    let bar_w = (slot_w * 0.52).clamp(8.0, 42.0);
    let base_y = pad_t + plot_h;

    for (idx, b) in bars.iter().enumerate() {
        let cx = pad_l + ((idx as f64) + 0.5) * slot_w;
        let bx = cx - bar_w * 0.5;
        let top_y = map_y(b.value);
        let bh = (base_y - top_y).max(1.5);
        let _ = writeln!(
            svg,
            r##"  <rect x="{bx:.1}" y="{top_y:.1}" width="{bar_w:.1}" height="{bh:.1}" fill="{}" rx="2"/>"##,
            b.color
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{cx:.1}" y="{:.1}" text-anchor="middle" font-family="ui-monospace, SFMono-Regular, monospace" font-size="8.5" fill="#1e293b">{}</text>"##,
            (top_y - 4.0).max(pad_t + 9.0),
            xml_escape(&b.display_value)
        );
        let _ = writeln!(
            svg,
            r##"  <text x="{cx:.1}" y="{:.1}" text-anchor="middle" font-family="ui-monospace, SFMono-Regular, monospace" font-size="9.5" fill="#475569">{}</text>"##,
            base_y + 15.0,
            xml_escape(&b.label)
        );
    }

    let _ = writeln!(svg, "</svg>");
    svg
}
