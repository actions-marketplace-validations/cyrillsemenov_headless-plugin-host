use crate::oversample::linear_to_db;
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OctaveRt60 {
    pub center_freq_hz: f64,
    pub rt60_sec: f64,
    pub r_squared: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rt60Report {
    pub sample_rate: f64,
    pub ir_frames: usize,
    pub dynamic_range_db: f64,
    pub edt_sec: f64,
    pub t20_sec: f64,
    pub t30_sec: f64,
    pub rt60_sec: f64,
    pub r_squared: f64,
    pub octave_bands: Vec<OctaveRt60>,
}

/// Apply a 2nd-order constant-skirt bandpass filter (1-octave bandwidth, Q = sqrt(2))
fn filter_bandpass_octave(samples: &[f32], sample_rate: f64, center_hz: f64) -> Vec<f32> {
    if center_hz <= 5.0 || center_hz >= sample_rate * 0.48 {
        return samples.to_vec();
    }
    let q = std::f64::consts::SQRT_2;
    let w0 = 2.0 * PI * center_hz / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = alpha;
    let b1 = 0.0;
    let b2 = -alpha;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    let b0 = b0 / a0;
    let b1 = b1 / a0;
    let b2 = b2 / a0;
    let a1 = a1 / a0;
    let a2 = a2 / a0;

    let mut out = Vec::with_capacity(samples.len());
    let mut z1 = 0.0f64;
    let mut z2 = 0.0f64;
    for &x_f32 in samples {
        let x = x_f32 as f64;
        let y = b0 * x + z1;
        z1 = b1 * x - a1 * y + z2;
        z2 = b2 * x - a2 * y;
        out.push(y as f32);
    }
    out
}

/// Perform least-squares linear regression of `edc_db[i]` vs `time_sec[i]` within `[top_db, bot_db]`
/// (where `top_db > bot_db`, e.g. `-5.0` to `-35.0`).
/// Returns `(extrapolated_rt60_sec, r_squared)`.
fn fit_edc_decay_slope(
    edc_db: &[f64],
    onset_idx: usize,
    sample_rate: f64,
    top_db: f64,
    bot_db: f64,
) -> (f64, f64) {
    let mut start = None;
    let mut end = None;
    for (i, &db) in edc_db.iter().enumerate().skip(onset_idx) {
        if start.is_none() && db <= top_db {
            start = Some(i);
        }
        if db <= bot_db {
            end = Some(i);
            break;
        }
    }

    let (s_idx, e_idx) = match (start, end) {
        (Some(s), Some(e)) if e > s + 2 => (s, e),
        _ => return (0.0, 0.0),
    };

    let count = (e_idx - s_idx + 1) as f64;
    let mut sum_t = 0.0;
    let mut sum_y = 0.0;
    let mut sum_tt = 0.0;
    let mut sum_ty = 0.0;
    let mut sum_yy = 0.0;

    for (i, &y) in edc_db[s_idx..=e_idx].iter().enumerate() {
        let t = ((s_idx + i - onset_idx) as f64) / sample_rate;
        sum_t += t;
        sum_y += y;
        sum_tt += t * t;
        sum_ty += t * y;
        sum_yy += y * y;
    }

    let denom = count * sum_tt - sum_t * sum_t;
    if denom.abs() < 1e-18 {
        return (0.0, 0.0);
    }

    let slope_db_per_sec = (count * sum_ty - sum_t * sum_y) / denom;
    if slope_db_per_sec >= -1e-6 {
        return (0.0, 0.0);
    }

    let rt60_sec = -60.0 / slope_db_per_sec;

    let ss_tot = sum_yy - (sum_y * sum_y) / count;
    let ss_reg = slope_db_per_sec * (sum_ty - (sum_t * sum_y) / count);
    let r_sq = if ss_tot > 1e-18 {
        (ss_reg / ss_tot).clamp(0.0, 1.0)
    } else {
        0.0
    };

    (rt60_sec, r_sq)
}

fn compute_schroeder_metrics(ir: &[f32], sample_rate: f64) -> (f64, f64, f64, f64, f64, f64) {
    if ir.len() < 16 {
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }

    // Find peak energy and onset
    let mut peak_sq = 0.0f64;
    let mut peak_idx = 0usize;
    for (i, &s) in ir.iter().enumerate() {
        if !s.is_finite() {
            continue;
        }
        let sq = (s as f64) * (s as f64);
        if sq > peak_sq {
            peak_sq = sq;
            peak_idx = i;
        }
    }
    if peak_sq <= 1e-24 {
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }

    // Find onset: first sample >= -40 dB of peak
    let onset_thresh = peak_sq * 1e-4;
    let onset_idx = ir
        .iter()
        .position(|&s| s.is_finite() && (s as f64) * (s as f64) >= onset_thresh)
        .unwrap_or(peak_idx);

    // Estimate tail noise floor from the last 10% of the buffer and find truncation limit (ISO 3382 Lundeby-style)
    let tail_start = (ir.len() * 9) / 10;
    let tail_slice = &ir[tail_start..];
    let tail_mean_sq = if !tail_slice.is_empty() {
        tail_slice
            .iter()
            .filter(|s| s.is_finite())
            .map(|&s| (s as f64) * (s as f64))
            .sum::<f64>()
            / (tail_slice.len() as f64)
    } else {
        0.0
    };

    // Smooth energy in 20ms blocks to find where the decay meets noise floor + 6 dB
    let win_len = ((0.020 * sample_rate).round() as usize).max(16);
    let cutoff_sq = (tail_mean_sq * 4.0).max(peak_sq * 1e-8);
    let mut trunc_idx = ir.len();
    if tail_mean_sq > peak_sq * 1e-6 && peak_idx + win_len < ir.len() {
        let mut i = peak_idx;
        while i + win_len <= ir.len() {
            let blk_sq = ir[i..i + win_len]
                .iter()
                .filter(|s| s.is_finite())
                .map(|&s| (s as f64) * (s as f64))
                .sum::<f64>()
                / (win_len as f64);
            if blk_sq <= cutoff_sq && i > onset_idx + win_len {
                trunc_idx = (i + win_len).min(ir.len());
                break;
            }
            i += win_len / 2;
        }
    }

    let valid_end = trunc_idx.max(onset_idx + 8).min(ir.len());
    let mut cum_energy = vec![0.0f64; valid_end];
    let mut acc = 0.0f64;
    for i in (0..valid_end).rev() {
        let s = ir[i];
        let sq = if s.is_finite() {
            ((s as f64) * (s as f64) - tail_mean_sq).max(0.0)
        } else {
            0.0
        };
        acc += sq;
        cum_energy[i] = acc;
    }

    // If the plugin mixes a direct dry impulse spike at `peak_idx` that drops > 3 dB within 2 ms,
    // reference the Schroeder decay curve to the reverberant tail onset right after the direct spike
    // so a 50% dry/wet mix doesn't skew T20/T30 by the dry-to-wet step.
    let skip_2ms = ((0.002 * sample_rate).round() as usize).max(4);
    let tail_probe_idx = (peak_idx + skip_2ms).min(valid_end.saturating_sub(4));
    let eval_onset = if tail_probe_idx > onset_idx
        && cum_energy[tail_probe_idx] > peak_sq * 1e-4
        && cum_energy[tail_probe_idx] < cum_energy[onset_idx] * 0.5
    {
        tail_probe_idx
    } else {
        onset_idx
    };

    let total_energy = cum_energy[eval_onset].max(1e-30);
    let edc_db: Vec<f64> = cum_energy
        .iter()
        .map(|&e| 10.0 * (e / total_energy).max(1e-12).log10())
        .collect();

    let min_edc_db = edc_db
        .iter()
        .skip(eval_onset)
        .copied()
        .fold(0.0f64, f64::min);
    let dynamic_range_db = -min_edc_db;

    let (edt_sec, edt_r2) = fit_edc_decay_slope(&edc_db, eval_onset, sample_rate, -0.1, -10.0);
    let (t20_sec, t20_r2) = fit_edc_decay_slope(&edc_db, eval_onset, sample_rate, -5.0, -25.0);
    let (t30_sec, t30_r2) = fit_edc_decay_slope(&edc_db, eval_onset, sample_rate, -5.0, -35.0);

    let (rt60_sec, r_squared) = if t30_sec > 0.0 && dynamic_range_db >= 35.0 && t30_r2 >= 0.80 {
        (t30_sec, t30_r2)
    } else if t20_sec > 0.0 && dynamic_range_db >= 25.0 {
        (t20_sec, t20_r2)
    } else {
        (edt_sec, edt_r2)
    };

    (
        dynamic_range_db,
        edt_sec,
        t20_sec,
        t30_sec,
        rt60_sec,
        r_squared,
    )
}

/// Measure ISO 3382 Reverberation Time (`EDT`, `T20`, `T30`, `RT60`, linearity `R^2`, and octave bands)
/// using Schroeder Backward Integration of an impulse response `ir`.
pub fn analyze_rt60(ir: &[f32], sample_rate: f64) -> Rt60Report {
    let (dynamic_range_db, edt_sec, t20_sec, t30_sec, rt60_sec, r_squared) =
        compute_schroeder_metrics(ir, sample_rate);

    let mut octave_bands = Vec::new();
    for &freq in &[
        63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ] {
        if freq >= sample_rate * 0.45 {
            continue;
        }
        let filtered = filter_bandpass_octave(ir, sample_rate, freq);
        let (_, _, _, _, band_rt60, band_r2) = compute_schroeder_metrics(&filtered, sample_rate);
        octave_bands.push(OctaveRt60 {
            center_freq_hz: freq,
            rt60_sec: band_rt60,
            r_squared: band_r2,
        });
    }

    Rt60Report {
        sample_rate,
        ir_frames: ir.len(),
        dynamic_range_db,
        edt_sec,
        t20_sec,
        t30_sec,
        rt60_sec,
        r_squared,
        octave_bands,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoTap {
    pub tap_index: usize,
    pub sample_index: usize,
    pub time_ms: f64,
    pub level_dbfs: f64,
    pub relative_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayReport {
    pub sample_rate: f64,
    pub onset_delay_samples: usize,
    pub onset_delay_ms: f64,
    pub peak_delay_samples: f64,
    pub peak_delay_ms: f64,
    pub xcorr_delay_samples: f64,
    pub xcorr_delay_ms: f64,
    pub reported_latency_samples: Option<u32>,
    pub latency_discrepancy_samples: Option<f64>,
    pub echo_taps: Vec<EchoTap>,
    pub estimated_tap_spacing_ms: Option<f64>,
    pub estimated_feedback_db_per_tap: Option<f64>,
}

/// Analyze true acoustic delay/latency, sub-sample cross-correlation delay, PDC latency discrepancy,
/// and multi-tap echo structure from an input/output signal pair (typically an impulse response).
pub fn analyze_delay_and_latency(
    input: &[f32],
    output: &[f32],
    sample_rate: f64,
    reported_latency: Option<u32>,
) -> DelayReport {
    let n = input.len().min(output.len());
    if n < 4 {
        return DelayReport {
            sample_rate,
            onset_delay_samples: 0,
            onset_delay_ms: 0.0,
            peak_delay_samples: 0.0,
            peak_delay_ms: 0.0,
            xcorr_delay_samples: 0.0,
            xcorr_delay_ms: 0.0,
            reported_latency_samples: reported_latency,
            latency_discrepancy_samples: None,
            echo_taps: Vec::new(),
            estimated_tap_spacing_ms: None,
            estimated_feedback_db_per_tap: None,
        };
    }

    // Find input impulse reference offset
    let mut in_peak_idx = 0usize;
    let mut in_peak_abs = 0.0f64;
    for (i, &x) in input[..n].iter().enumerate() {
        let a = (x as f64).abs();
        if a > in_peak_abs {
            in_peak_abs = a;
            in_peak_idx = i;
        }
    }

    // Find output peak and parabolic sub-sample interpolation
    let mut out_peak_idx = 0usize;
    let mut out_peak_abs = 0.0f64;
    for (i, &y) in output[..n].iter().enumerate() {
        if !y.is_finite() {
            continue;
        }
        let a = (y as f64).abs();
        if a > out_peak_abs {
            out_peak_abs = a;
            out_peak_idx = i;
        }
    }

    let frac_peak = if out_peak_idx > 0 && out_peak_idx + 1 < n {
        let y0 = (output[out_peak_idx - 1] as f64).abs();
        let y1 = (output[out_peak_idx] as f64).abs();
        let y2 = (output[out_peak_idx + 1] as f64).abs();
        let denom = y0 - 2.0 * y1 + y2;
        let delta = if denom.abs() > 1e-12 {
            0.5 * (y0 - y2) / denom
        } else {
            0.0
        };
        (out_peak_idx as f64) + delta.clamp(-0.5, 0.5)
    } else {
        out_peak_idx as f64
    };

    let peak_delay_samples = (frac_peak - (in_peak_idx as f64)).max(0.0);
    let peak_delay_ms = peak_delay_samples * 1000.0 / sample_rate;

    // Onset sample: first sample exceeding -40 dB below output peak
    let onset_thresh = out_peak_abs * 0.01;
    let out_onset_idx = output[..n]
        .iter()
        .position(|&y| y.is_finite() && (y as f64).abs() >= onset_thresh)
        .unwrap_or(out_peak_idx);
    let onset_delay_samples = out_onset_idx.saturating_sub(in_peak_idx);
    let onset_delay_ms = (onset_delay_samples as f64) * 1000.0 / sample_rate;

    // FFT Cross-Correlation (blended GCC-PHAT + standard energy cross-correlation)
    let fft_len = (n * 2).next_power_of_two().max(64);
    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(fft_len);
    let c2r = planner.plan_fft_inverse(fft_len);

    let mut x_pad = vec![0.0f64; fft_len];
    let mut y_pad = vec![0.0f64; fft_len];
    for i in 0..n {
        x_pad[i] = if input[i].is_finite() {
            input[i] as f64
        } else {
            0.0
        };
        y_pad[i] = if output[i].is_finite() {
            output[i] as f64
        } else {
            0.0
        };
    }

    let mut x_spec = r2c.make_output_vec();
    let mut y_spec = r2c.make_output_vec();
    let _ = r2c.process(&mut x_pad, &mut x_spec);
    let _ = r2c.process(&mut y_pad, &mut y_spec);

    let mut cross_spec: Vec<Complex<f64>> = x_spec
        .iter()
        .zip(y_spec.iter())
        .map(|(&x, &y)| {
            let prod = y * x.conj();
            // Blended PHAT weighting (sqrt(mag)) preserves impulse sharpness while staying immune to noise bins
            let weight = prod.norm().sqrt().max(1e-12);
            prod / weight
        })
        .collect();
    if !cross_spec.is_empty() {
        cross_spec[0].im = 0.0;
    }
    if cross_spec.len() > 1 && fft_len.is_multiple_of(2) {
        if let Some(last) = cross_spec.last_mut() {
            last.im = 0.0;
        }
    }

    let mut xcorr = c2r.make_output_vec();
    let _ = c2r.process(&mut cross_spec, &mut xcorr);

    let mut best_lag = 0usize;
    let mut best_xc = 0.0f64;
    for (lag, &val) in xcorr.iter().take(n).enumerate() {
        let a = val.abs();
        if a > best_xc {
            best_xc = a;
            best_lag = lag;
        }
    }

    let xcorr_delay_samples = if best_lag > 0 && best_lag + 1 < n {
        let c0 = xcorr[best_lag - 1].abs();
        let c1 = xcorr[best_lag].abs();
        let c2 = xcorr[best_lag + 1].abs();
        let denom = c0 - 2.0 * c1 + c2;
        let delta = if denom.abs() > 1e-12 {
            0.5 * (c0 - c2) / denom
        } else {
            0.0
        };
        (best_lag as f64) + delta.clamp(-0.5, 0.5)
    } else {
        best_lag as f64
    };
    let xcorr_delay_ms = xcorr_delay_samples * 1000.0 / sample_rate;

    // Multi-tap Echo Finder: detect discrete local peaks above -60 dB of main peak, separated by at least 2 ms
    let min_sep_samples = ((0.002 * sample_rate).round() as usize).max(4);
    let min_tap_amp = out_peak_abs * 1e-3; // -60 dBc
    let mut raw_peaks: Vec<(usize, f64)> = Vec::new();

    for i in 0..n {
        let a = if output[i].is_finite() {
            (output[i] as f64).abs()
        } else {
            0.0
        };
        if a < min_tap_amp {
            continue;
        }
        let prev = if i > 0 {
            (output[i - 1] as f64).abs()
        } else {
            0.0
        };
        let next = if i + 1 < n {
            (output[i + 1] as f64).abs()
        } else {
            0.0
        };
        if a >= prev && a >= next {
            if let Some(last) = raw_peaks.last_mut() {
                if i.saturating_sub(last.0) < min_sep_samples {
                    if a > last.1 {
                        *last = (i, a);
                    }
                    continue;
                }
            }
            raw_peaks.push((i, a));
        }
    }

    let peak_db = linear_to_db(out_peak_abs);
    let echo_taps: Vec<EchoTap> = raw_peaks
        .iter()
        .take(16)
        .enumerate()
        .map(|(idx, &(sample_idx, amp))| {
            let rel_samples = sample_idx.saturating_sub(in_peak_idx);
            let lvl = linear_to_db(amp);
            EchoTap {
                tap_index: idx,
                sample_index: rel_samples,
                time_ms: (rel_samples as f64) * 1000.0 / sample_rate,
                level_dbfs: lvl,
                relative_db: lvl - peak_db,
            }
        })
        .collect();

    let (estimated_tap_spacing_ms, estimated_feedback_db_per_tap) = if echo_taps.len() >= 2 {
        let mut dt_sum = 0.0;
        let mut db_sum = 0.0;
        let pairs = (echo_taps.len() - 1) as f64;
        for w in echo_taps.windows(2) {
            dt_sum += w[1].time_ms - w[0].time_ms;
            db_sum += w[1].level_dbfs - w[0].level_dbfs;
        }
        (Some(dt_sum / pairs), Some(db_sum / pairs))
    } else {
        (None, None)
    };

    let latency_discrepancy_samples =
        reported_latency.map(|rep| (onset_delay_samples as f64) - (rep as f64));

    DelayReport {
        sample_rate,
        onset_delay_samples,
        onset_delay_ms,
        peak_delay_samples,
        peak_delay_ms,
        xcorr_delay_samples,
        xcorr_delay_ms,
        reported_latency_samples: reported_latency,
        latency_discrepancy_samples,
        echo_taps,
        estimated_tap_spacing_ms,
        estimated_feedback_db_per_tap,
    }
}
