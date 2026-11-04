use crate::oversample::linear_to_db;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicsCurvePoint {
    pub input_dbfs: f64,
    pub output_dbfs: f64,
    pub gain_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicsReport {
    pub sample_rate: f64,
    pub static_curve: Vec<DynamicsCurvePoint>,
    pub estimated_ratio: f64,
    pub estimated_threshold_dbfs: Option<f64>,
    pub attack_time_ms: f64,
    pub release_time_ms: f64,
    pub max_gain_reduction_db: f64,
}

/// Measure compressor/limiter static transfer curve (`input_dbfs -> output_dbfs`), compression ratio,
/// threshold knee, and tone-burst `10%..90%` attack & release times in milliseconds.
///
/// `render_mono_fn` takes a mono `f32` stimulus slice and returns the rendered mono `f32` output from a freshly reset plugin.
pub fn analyze_dynamics_envelope<F>(sample_rate: f64, mut render_mono_fn: F) -> DynamicsReport
where
    F: FnMut(&[f32]) -> Vec<f32>,
{
    let step_levels_db: &[f64] = &[
        -48.0, -36.0, -24.0, -18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0, 24.0,
    ];
    let step_frames = ((0.25 * sample_rate).round() as usize).max(1024);
    let mut static_curve = Vec::with_capacity(step_levels_db.len());

    for &in_db in step_levels_db {
        let amp = 10.0f64.powf(in_db / 20.0);
        let w = 2.0 * PI * 1000.0 / sample_rate;
        let stim: Vec<f32> = (0..step_frames)
            .map(|n| (amp * (w * (n as f64)).sin()) as f32)
            .collect();
        let out = render_mono_fn(&stim);
        let steady = &out[out.len() / 2..];
        let rms = if !steady.is_empty() {
            (steady
                .iter()
                .filter(|s| s.is_finite())
                .map(|&s| (s as f64) * (s as f64))
                .sum::<f64>()
                / (steady.len() as f64))
                .sqrt()
        } else {
            0.0
        };
        // Peak equivalent of sine RMS is rms * sqrt(2)
        let out_db = linear_to_db(rms * std::f64::consts::SQRT_2);
        static_curve.push(DynamicsCurvePoint {
            input_dbfs: in_db,
            output_dbfs: out_db,
            gain_db: out_db - in_db,
        });
    }

    // Estimate linear gain at -36 dBFS, threshold (where gain drops by >= 1.5 dB), and high-level ratio
    let ref_gain_db = static_curve
        .iter()
        .find(|p| (p.input_dbfs - (-36.0)).abs() < 0.1)
        .or_else(|| static_curve.first())
        .map(|p| p.gain_db)
        .unwrap_or(0.0);
    let estimated_threshold_dbfs = static_curve
        .iter()
        .find(|p| p.input_dbfs >= -36.0 && (p.gain_db - ref_gain_db) <= -1.5)
        .map(|p| p.input_dbfs);

    let pt_0db = static_curve
        .iter()
        .find(|p| p.input_dbfs.abs() < 0.1)
        .or_else(|| static_curve.last());
    let pt_m12db = static_curve
        .iter()
        .find(|p| (p.input_dbfs - (-12.0)).abs() < 0.1)
        .or_else(|| static_curve.first());
    let estimated_ratio = match (pt_0db, pt_m12db) {
        (Some(hi), Some(mid)) if (hi.input_dbfs - mid.input_dbfs).abs() > 1.0 => {
            let d_in = (hi.input_dbfs - mid.input_dbfs).abs();
            let d_out = (hi.output_dbfs - mid.output_dbfs).abs().max(0.05);
            (d_in / d_out).clamp(0.1, 100.0)
        }
        _ => 1.0,
    };

    // Tone burst test for Attack & Release time:
    // Segment 1 (0..150 ms): -24 dBFS
    // Segment 2 (150..400 ms): -2 dBFS (loud burst)
    // Segment 3 (400..750 ms): -24 dBFS (recovery)
    let n1 = (0.150 * sample_rate).round() as usize;
    let n2 = (0.400 * sample_rate).round() as usize;
    let n3 = (0.750 * sample_rate).round() as usize;
    let amp_low = 10.0f64.powf(-24.0 / 20.0);
    let amp_high = 10.0f64.powf(-2.0 / 20.0);
    let w = 2.0 * PI * 1000.0 / sample_rate;

    let mut burst = Vec::with_capacity(n3);
    for i in 0..n3 {
        let a = if i >= n1 && i < n2 { amp_high } else { amp_low };
        burst.push((a * (w * (i as f64)).sin()) as f32);
    }

    let burst_out = render_mono_fn(&burst);
    // Compute smoothed instant gain envelope (1 ms window = 1 cycle of 1 kHz)
    let cycle_len = ((sample_rate / 1000.0).round() as usize).max(8);
    let mut env_gain_db = vec![0.0f64; burst_out.len()];
    for (i, env_db) in env_gain_db.iter_mut().enumerate() {
        let start = i.saturating_sub(cycle_len / 2);
        let end = (i + cycle_len / 2).min(burst_out.len());
        let in_rms = (burst[start..end]
            .iter()
            .map(|&x| (x as f64) * (x as f64))
            .sum::<f64>()
            / ((end - start).max(1) as f64))
            .sqrt()
            .max(1e-6);
        let out_rms = (burst_out[start..end]
            .iter()
            .filter(|x| x.is_finite())
            .map(|&x| (x as f64) * (x as f64))
            .sum::<f64>()
            / ((end - start).max(1) as f64))
            .sqrt();
        *env_db = linear_to_db(out_rms / in_rms);
    }

    let baseline_gain = if n1 > cycle_len * 2 {
        env_gain_db[n1 - cycle_len * 2]
    } else {
        ref_gain_db
    };
    let settled_burst_gain = if n2 > cycle_len * 2 {
        env_gain_db[n2 - cycle_len * 2]
    } else {
        baseline_gain
    };

    let gr_db = (baseline_gain - settled_burst_gain).max(0.0);

    let (attack_time_ms, release_time_ms) = if gr_db >= 0.5 {
        let atk_10 = baseline_gain - 0.10 * gr_db;
        let atk_90 = baseline_gain - 0.90 * gr_db;
        let t_atk_start = env_gain_db[n1..n2]
            .iter()
            .position(|&g| g <= atk_10)
            .unwrap_or(0);
        let t_atk_end = env_gain_db[n1..n2]
            .iter()
            .position(|&g| g <= atk_90)
            .unwrap_or(t_atk_start);
        let atk_ms = (t_atk_end.saturating_sub(t_atk_start) as f64) * 1000.0 / sample_rate;

        let rel_10 = settled_burst_gain + 0.10 * gr_db;
        let rel_90 = settled_burst_gain + 0.90 * gr_db;
        let t_rel_start = env_gain_db[n2..n3]
            .iter()
            .position(|&g| g >= rel_10)
            .unwrap_or(0);
        let t_rel_end = env_gain_db[n2..n3]
            .iter()
            .position(|&g| g >= rel_90)
            .unwrap_or(t_rel_start);
        let rel_ms = (t_rel_end.saturating_sub(t_rel_start) as f64) * 1000.0 / sample_rate;
        (atk_ms, rel_ms)
    } else {
        (0.0, 0.0)
    };

    DynamicsReport {
        sample_rate,
        static_curve,
        estimated_ratio,
        estimated_threshold_dbfs,
        attack_time_ms,
        release_time_ms,
        max_gain_reduction_db: gr_db,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StereoReport {
    pub phase_correlation: f64,
    pub lr_balance_db: f64,
    pub mid_rms_dbfs: f64,
    pub side_rms_dbfs: f64,
    pub mid_side_ratio_db: f64,
    pub stereo_width: f64,
}

/// Analyze stereo phase correlation (`-1.0 .. +1.0`), L/R balance (`dB`), Mid/Side RMS (`dBFS`),
/// and stereo width (`0.0 = mono`, `0.5 = uncorrelated`, `1.0 = pure anti-phase side`).
pub fn analyze_stereo_image(left: &[f32], right: &[f32]) -> StereoReport {
    let n = left.len().min(right.len());
    if n == 0 {
        return StereoReport {
            phase_correlation: 1.0,
            lr_balance_db: 0.0,
            mid_rms_dbfs: -400.0,
            side_rms_dbfs: -400.0,
            mid_side_ratio_db: 0.0,
            stereo_width: 0.0,
        };
    }

    let mut sum_ll = 0.0f64;
    let mut sum_rr = 0.0f64;
    let mut sum_lr = 0.0f64;
    let mut sum_mid_sq = 0.0f64;
    let mut sum_side_sq = 0.0f64;

    for i in 0..n {
        let l = if left[i].is_finite() {
            left[i] as f64
        } else {
            0.0
        };
        let r = if right[i].is_finite() {
            right[i] as f64
        } else {
            0.0
        };
        sum_ll += l * l;
        sum_rr += r * r;
        sum_lr += l * r;

        let mid = 0.5 * (l + r);
        let side = 0.5 * (l - r);
        sum_mid_sq += mid * mid;
        sum_side_sq += side * side;
    }

    let denom = (sum_ll * sum_rr).sqrt();
    let phase_correlation = if denom > 1e-24 {
        (sum_lr / denom).clamp(-1.0, 1.0)
    } else {
        1.0
    };

    let nf = n as f64;
    let l_rms = (sum_ll / nf).sqrt();
    let r_rms = (sum_rr / nf).sqrt();
    let mid_rms = (sum_mid_sq / nf).sqrt();
    let side_rms = (sum_side_sq / nf).sqrt();

    let lr_balance_db = linear_to_db(l_rms) - linear_to_db(r_rms);
    let mid_rms_dbfs = linear_to_db(mid_rms);
    let side_rms_dbfs = linear_to_db(side_rms);
    let mid_side_ratio_db = mid_rms_dbfs - side_rms_dbfs;
    let stereo_width = if mid_rms + side_rms > 1e-20 {
        (side_rms / (mid_rms + side_rms)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    StereoReport {
        phase_correlation,
        lr_balance_db,
        mid_rms_dbfs,
        side_rms_dbfs,
        mid_side_ratio_db,
        stereo_width,
    }
}
