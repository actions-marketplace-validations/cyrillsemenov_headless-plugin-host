use crate::oversample::linear_to_db;
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowType {
    Rectangular,
    Hann,
    BlackmanHarris,
    FlatTop,
}

pub fn make_window(window: WindowType, len: usize) -> Vec<f64> {
    if len <= 1 {
        return vec![1.0; len];
    }
    let n_minus_1 = (len - 1) as f64;
    (0..len)
        .map(|i| {
            let x = (i as f64) / n_minus_1;
            match window {
                WindowType::Rectangular => 1.0,
                WindowType::Hann => 0.5 * (1.0 - (2.0 * PI * x).cos()),
                WindowType::BlackmanHarris => {
                    0.35875 - 0.48829 * (2.0 * PI * x).cos() + 0.14128 * (4.0 * PI * x).cos()
                        - 0.01168 * (6.0 * PI * x).cos()
                }
                WindowType::FlatTop => {
                    0.21557895 - 0.41663158 * (2.0 * PI * x).cos()
                        + 0.277263158 * (4.0 * PI * x).cos()
                        - 0.083578947 * (6.0 * PI * x).cos()
                        + 0.006947368 * (8.0 * PI * x).cos()
                }
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralPeak {
    pub freq_hz: f64,
    pub magnitude_db: f64,
    pub bin: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumReport {
    pub fft_size: usize,
    pub sample_rate: f64,
    pub bin_resolution_hz: f64,
    pub peak_freq_hz: f64,
    pub peak_magnitude_db: f64,
    pub spectral_centroid_hz: f64,
    pub spectral_flatness: f64,
    pub top_peaks: Vec<SpectralPeak>,
}

pub fn compute_spectrum(
    samples: &[f32],
    sample_rate: f64,
    window_type: WindowType,
) -> SpectrumReport {
    let n = samples.len().max(2);
    let win = make_window(window_type, n);
    let coherent_gain: f64 = win.iter().sum::<f64>() / (n as f64);

    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(n);
    let mut input: Vec<f64> = samples
        .iter()
        .zip(win.iter())
        .map(|(&s, &w)| (s as f64) * w)
        .collect();
    input.resize(n, 0.0);

    let mut spectrum = r2c.make_output_vec();
    let _ = r2c.process(&mut input, &mut spectrum);

    let norm = 2.0 / ((n as f64) * coherent_gain.max(1e-12));
    let bin_hz = sample_rate / (n as f64);

    let mut mags_lin = Vec::with_capacity(spectrum.len());
    let mut mags_db = Vec::with_capacity(spectrum.len());

    let mut num_centroid = 0.0;
    let mut den_centroid = 0.0;
    let mut log_sum = 0.0;
    let mut arith_sum = 0.0;

    let mut best_bin = 0;
    let mut best_mag_db = -400.0;

    for (bin, c) in spectrum.iter().enumerate() {
        let scale = if bin == 0 || bin == n / 2 {
            norm * 0.5
        } else {
            norm
        };
        let mag = c.norm() * scale;
        let db = linear_to_db(mag);
        let freq = (bin as f64) * bin_hz;

        mags_lin.push(mag);
        mags_db.push(db);

        if bin > 0 {
            num_centroid += freq * mag;
            den_centroid += mag;
            let p = (mag * mag).max(1e-30);
            log_sum += p.ln();
            arith_sum += p;
            if db > best_mag_db {
                best_mag_db = db;
                best_bin = bin;
            }
        }
    }

    // Parabolic interpolation around best_bin for sub-bin frequency accuracy
    let peak_freq_hz = if best_bin > 0 && best_bin + 1 < mags_db.len() {
        let a = mags_db[best_bin - 1];
        let b = mags_db[best_bin];
        let g = mags_db[best_bin + 1];
        let denom = a - 2.0 * b + g;
        let delta = if denom.abs() > 1e-12 {
            0.5 * (a - g) / denom
        } else {
            0.0
        };
        ((best_bin as f64) + delta.clamp(-0.5, 0.5)) * bin_hz
    } else {
        (best_bin as f64) * bin_hz
    };

    // Collect local maxima for top_peaks with parabolic sub-bin interpolation
    let mut peaks = Vec::new();
    for bin in 1..mags_db.len().saturating_sub(1) {
        if mags_db[bin] > mags_db[bin - 1]
            && mags_db[bin] >= mags_db[bin + 1]
            && mags_db[bin] > -140.0
        {
            let a = mags_db[bin - 1];
            let b = mags_db[bin];
            let g = mags_db[bin + 1];
            let denom = a - 2.0 * b + g;
            let delta = if denom.abs() > 1e-12 {
                0.5 * (a - g) / denom
            } else {
                0.0
            };
            peaks.push(SpectralPeak {
                freq_hz: ((bin as f64) + delta.clamp(-0.5, 0.5)) * bin_hz,
                magnitude_db: mags_db[bin],
                bin,
            });
        }
    }
    peaks.sort_by(|a, b| b.magnitude_db.total_cmp(&a.magnitude_db));
    peaks.truncate(10);

    let count_bins = (spectrum.len().saturating_sub(1)).max(1) as f64;
    let geom_mean = (log_sum / count_bins).exp();
    let arith_mean = (arith_sum / count_bins).max(1e-30);

    SpectrumReport {
        fft_size: n,
        sample_rate,
        bin_resolution_hz: bin_hz,
        peak_freq_hz,
        peak_magnitude_db: best_mag_db,
        spectral_centroid_hz: if den_centroid > 1e-20 {
            num_centroid / den_centroid
        } else {
            0.0
        },
        spectral_flatness: (geom_mean / arith_mean).clamp(0.0, 1.0),
        top_peaks: peaks,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonicSpur {
    pub order: usize,
    pub freq_hz: f64,
    pub level_dbc: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThdNReport {
    pub fundamental_freq_hz: f64,
    pub fundamental_level_dbfs: f64,
    pub thd_percent: f64,
    pub thd_db: f64,
    pub even_thd_db: f64,
    pub odd_thd_db: f64,
    pub thdn_percent: f64,
    pub thdn_db: f64,
    pub sinad_db: f64,
    pub harmonics: Vec<HarmonicSpur>,
    pub max_alias_spur_dbc: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImdSpur {
    pub label: String,
    pub freq_hz: f64,
    pub level_dbc: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImdReport {
    pub f1_hz: f64,
    pub f2_hz: f64,
    pub f1_level_dbfs: f64,
    pub f2_level_dbfs: f64,
    pub imd_percent: f64,
    pub imd_db: f64,
    pub diff_tone_dbc: f64,
    pub third_order_lower_dbc: f64,
    pub third_order_upper_dbc: f64,
    pub sidebands: Vec<ImdSpur>,
}

/// Measure Total Harmonic Distortion + Noise (THD+N), Even/Odd harmonic balance, harmonic profile (H2..H10), and non-harmonic alias spurs.
pub fn analyze_thdn(samples: &[f32], sample_rate: f64, fundamental_hz: f64) -> ThdNReport {
    let n = samples.len().max(64);
    let win = make_window(WindowType::BlackmanHarris, n);
    let coherent_gain: f64 = win.iter().sum::<f64>() / (n as f64);

    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(n);
    let mut input: Vec<f64> = samples
        .iter()
        .zip(win.iter())
        .map(|(&s, &w)| (s as f64) * w)
        .collect();
    let mut spec = r2c.make_output_vec();
    let _ = r2c.process(&mut input, &mut spec);

    let norm = 2.0 / ((n as f64) * coherent_gain.max(1e-12));
    let bin_hz = sample_rate / (n as f64);
    let min_bin = ((20.0 / bin_hz).ceil() as usize).max(1);
    let nominal_fund_bin = ((fundamental_hz / bin_hz).round() as usize).clamp(1, spec.len() - 1);

    // Blackman-Harris mainlobe half-width is 4 bins
    let lobe_radius = 4usize;

    let bin_power = |center_bin: usize| -> f64 {
        let start = center_bin.saturating_sub(lobe_radius).max(1);
        let end = (center_bin + lobe_radius).min(spec.len() - 1);
        let mut p = 0.0;
        for c in &spec[start..=end] {
            let m = c.norm() * norm;
            p += m * m;
        }
        p
    };

    // Auto-track fundamental if plugin pitch-shifts, quantizes pitch (e.g. Auto-Tune), or runs an internal oscillator
    let nominal_power = bin_power(nominal_fund_bin);
    let mut best_bin = nominal_fund_bin;
    let mut best_bin_mag_sq = 0.0f64;
    for (b, c) in spec.iter().enumerate().skip(min_bin) {
        let m = c.norm() * norm;
        let p = m * m;
        if p > best_bin_mag_sq {
            best_bin_mag_sq = p;
            best_bin = b;
        }
    }
    let best_lobe_power = bin_power(best_bin);
    let (fund_bin, actual_fundamental_hz) =
        if best_lobe_power > 1e-8 && nominal_power < best_lobe_power * 0.1 {
            (best_bin, (best_bin as f64) * bin_hz)
        } else {
            (nominal_fund_bin, fundamental_hz)
        };

    let fund_power = bin_power(fund_bin).max(1e-30);
    let fund_amp = fund_power.sqrt();

    let mut harmonics = Vec::new();
    let mut harmonic_power_sum = 0.0;
    let mut even_harmonic_power_sum = 0.0;
    let mut odd_harmonic_power_sum = 0.0;
    let mut harmonic_bins_mask = vec![false; spec.len()];

    // Mask DC and Fundamental bins
    let dc_end = lobe_radius.min(spec.len() - 1);
    harmonic_bins_mask[..=dc_end].fill(true);
    let fund_start = fund_bin.saturating_sub(lobe_radius);
    let fund_end = (fund_bin + lobe_radius).min(spec.len() - 1);
    harmonic_bins_mask[fund_start..=fund_end].fill(true);

    for order in 2..=10 {
        let h_freq = actual_fundamental_hz * (order as f64);
        if h_freq >= sample_rate * 0.5 {
            break;
        }
        let h_bin = (h_freq / bin_hz).round() as usize;
        if h_bin < spec.len() {
            let hp = bin_power(h_bin);
            harmonic_power_sum += hp;
            if order % 2 == 0 {
                even_harmonic_power_sum += hp;
            } else {
                odd_harmonic_power_sum += hp;
            }
            let level_dbc = 10.0 * (hp / fund_power).max(1e-30).log10();
            harmonics.push(HarmonicSpur {
                order,
                freq_hz: h_freq,
                level_dbc,
            });
            let h_start = h_bin.saturating_sub(lobe_radius);
            let h_end = (h_bin + lobe_radius).min(spec.len() - 1);
            harmonic_bins_mask[h_start..=h_end].fill(true);
        }
    }

    // Total non-fundamental power (20 Hz .. Nyquist) and max non-harmonic alias spur
    let mut thdn_power = 0.0;
    let mut max_alias_power: f64 = 1e-30;

    for (b, c) in spec.iter().enumerate().skip(min_bin) {
        if b >= fund_bin.saturating_sub(lobe_radius) && b <= fund_bin + lobe_radius {
            continue;
        }
        let m = c.norm() * norm;
        let p = m * m;
        thdn_power += p;
        if !harmonic_bins_mask[b] && p > max_alias_power {
            max_alias_power = p;
        }
    }

    let total_power = fund_power + thdn_power;
    let (thd_ratio, even_thd_ratio, odd_thd_ratio, thdn_ratio) = if total_power < 1e-12 {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        let thd_denom = if harmonic_power_sum <= fund_power {
            fund_power
        } else {
            fund_power + harmonic_power_sum
        };
        let thdn_denom = if thdn_power <= fund_power {
            fund_power
        } else {
            total_power
        };
        (
            (harmonic_power_sum / thd_denom).sqrt(),
            (even_harmonic_power_sum / thd_denom).sqrt(),
            (odd_harmonic_power_sum / thd_denom).sqrt(),
            (thdn_power / thdn_denom).sqrt(),
        )
    };
    let thd_db = linear_to_db(thd_ratio);
    let even_thd_db = linear_to_db(even_thd_ratio);
    let odd_thd_db = linear_to_db(odd_thd_ratio);
    let thdn_db = linear_to_db(thdn_ratio);

    ThdNReport {
        fundamental_freq_hz: actual_fundamental_hz,
        fundamental_level_dbfs: linear_to_db(fund_amp),
        thd_percent: thd_ratio * 100.0,
        thd_db,
        even_thd_db,
        odd_thd_db,
        thdn_percent: thdn_ratio * 100.0,
        thdn_db,
        sinad_db: -thdn_db,
        harmonics,
        max_alias_spur_dbc: 10.0 * (max_alias_power / fund_power.max(total_power)).log10(),
    }
}

/// Measure Two-Tone Intermodulation Distortion (`IMD` — both CCIF difference-frequency & SMPTE sidebands).
/// Evaluates `|f2 - f1|`, `f1 + f2`, `2*f1 - f2`, `2*f2 - f1`, and `f2 ± k*f1` products.
pub fn analyze_imd(samples: &[f32], sample_rate: f64, f1_hz: f64, f2_hz: f64) -> ImdReport {
    let n = samples.len().max(64);
    let win = make_window(WindowType::BlackmanHarris, n);
    let coherent_gain: f64 = win.iter().sum::<f64>() / (n as f64);

    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(n);
    let mut input: Vec<f64> = samples
        .iter()
        .zip(win.iter())
        .map(|(&s, &w)| (s as f64) * w)
        .collect();
    let mut spec = r2c.make_output_vec();
    let _ = r2c.process(&mut input, &mut spec);

    let norm = 2.0 / ((n as f64) * coherent_gain.max(1e-12));
    let bin_hz = sample_rate / (n as f64);
    let lobe_radius = 4usize;

    let power_at_freq = |f_hz: f64| -> f64 {
        if f_hz <= 5.0 || f_hz >= sample_rate * 0.495 {
            return 0.0;
        }
        let center_bin = ((f_hz / bin_hz).round() as usize).clamp(1, spec.len() - 1);
        let start = center_bin.saturating_sub(lobe_radius).max(1);
        let end = (center_bin + lobe_radius).min(spec.len() - 1);
        let mut p = 0.0;
        for c in &spec[start..=end] {
            let m = c.norm() * norm;
            p += m * m;
        }
        p
    };

    let f_low = f1_hz.min(f2_hz);
    let f_high = f1_hz.max(f2_hz);
    let p1 = power_at_freq(f_low).max(1e-30);
    let p2 = power_at_freq(f_high).max(1e-30);
    let ref_power = (p1 + p2).max(1e-30);

    let mut sidebands = Vec::new();
    let mut imd_power_sum = 0.0;

    // 2nd-order difference (f2 - f1) & sum (f2 + f1)
    let f_diff = (f_high - f_low).abs();
    let p_diff = power_at_freq(f_diff);
    imd_power_sum += p_diff;
    let diff_tone_dbc = 10.0 * (p_diff / ref_power).max(1e-30).log10();
    sidebands.push(ImdSpur {
        label: "f2 - f1 (IMD2-)".into(),
        freq_hz: f_diff,
        level_dbc: diff_tone_dbc,
    });

    let f_sum = f_high + f_low;
    if f_sum < sample_rate * 0.49 {
        let p_sum = power_at_freq(f_sum);
        imd_power_sum += p_sum;
        sidebands.push(ImdSpur {
            label: "f2 + f1 (IMD2+)".into(),
            freq_hz: f_sum,
            level_dbc: 10.0 * (p_sum / ref_power).max(1e-30).log10(),
        });
    }

    // 3rd-order intermodulation: 2*f1 - f2 and 2*f2 - f1
    let f_im3_lo = (2.0 * f_low - f_high).abs();
    let p_im3_lo =
        if (f_im3_lo - f_low).abs() > bin_hz * 10.0 && (f_im3_lo - f_high).abs() > bin_hz * 10.0 {
            power_at_freq(f_im3_lo)
        } else {
            0.0
        };
    imd_power_sum += p_im3_lo;
    let third_order_lower_dbc = 10.0 * (p_im3_lo / ref_power).max(1e-30).log10();
    sidebands.push(ImdSpur {
        label: "2*f1 - f2 (IMD3-)".into(),
        freq_hz: f_im3_lo,
        level_dbc: third_order_lower_dbc,
    });

    let f_im3_hi = 2.0 * f_high - f_low;
    let p_im3_hi = if f_im3_hi < sample_rate * 0.49 {
        power_at_freq(f_im3_hi)
    } else {
        0.0
    };
    imd_power_sum += p_im3_hi;
    let third_order_upper_dbc = 10.0 * (p_im3_hi / ref_power).max(1e-30).log10();
    if f_im3_hi < sample_rate * 0.49 {
        sidebands.push(ImdSpur {
            label: "2*f2 - f1 (IMD3+)".into(),
            freq_hz: f_im3_hi,
            level_dbc: third_order_upper_dbc,
        });
    }

    // SMPTE-style sidebands around f_high: f_high +/- 2*f_low, f_high +/- 3*f_low
    if f_high > 4.0 * f_low {
        for k in 2..=3 {
            for sign in [-1.0f64, 1.0f64] {
                let f_sb = f_high + sign * (k as f64) * f_low;
                if f_sb > 20.0 && f_sb < sample_rate * 0.49 {
                    let p_sb = power_at_freq(f_sb);
                    imd_power_sum += p_sb;
                    let sign_ch = if sign < 0.0 { '-' } else { '+' };
                    sidebands.push(ImdSpur {
                        label: format!("f2 {sign_ch} {k}*f1"),
                        freq_hz: f_sb,
                        level_dbc: 10.0 * (p_sb / p2).max(1e-30).log10(),
                    });
                }
            }
        }
    }

    let imd_ratio = (imd_power_sum / ref_power).sqrt();
    let imd_db = linear_to_db(imd_ratio);

    ImdReport {
        f1_hz: f_low,
        f2_hz: f_high,
        f1_level_dbfs: linear_to_db(p1.sqrt()),
        f2_level_dbfs: linear_to_db(p2.sqrt()),
        imd_percent: imd_ratio * 100.0,
        imd_db,
        diff_tone_dbc,
        third_order_lower_dbc,
        third_order_upper_dbc,
        sidebands,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodePoint {
    pub freq_hz: f64,
    pub magnitude_db: f64,
    pub phase_deg: f64,
    pub group_delay_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodeReport {
    pub points: Vec<BodePoint>,
    pub dc_gain_db: f64,
    pub gain_1khz_db: f64,
    pub cutoff_minus_3db_hz: Option<f64>,
    pub max_gain_db: f64,
    pub min_gain_db: f64,
}

/// Extract transfer function H(f) = Y(f) / X(f) from input & output signals (e.g. Impulse or Sweep).
pub fn compute_bode_response(
    input_signal: &[f32],
    output_signal: &[f32],
    sample_rate: f64,
) -> BodeReport {
    let n = input_signal.len().min(output_signal.len()).max(64);
    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(n);

    let mut x_buf: Vec<f64> = input_signal[..n].iter().map(|&v| v as f64).collect();
    let mut y_buf: Vec<f64> = output_signal[..n].iter().map(|&v| v as f64).collect();
    let mut x_spec = r2c.make_output_vec();
    let mut y_spec = r2c.make_output_vec();

    let _ = r2c.process(&mut x_buf, &mut x_spec);
    let _ = r2c.process(&mut y_buf, &mut y_spec);

    let bin_hz = sample_rate / (n as f64);

    // Standard 31-band ISO 1/3-octave frequencies (20 Hz .. 20 kHz)
    let probe_freqs: &[f64] = &[
        20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
        500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0,
        6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
    ];

    let complex_transfer_at_bin = |bin: usize| -> Complex<f64> {
        let x: Complex<f64> = x_spec[bin];
        let y: Complex<f64> = y_spec[bin];
        let denom = x.norm_sqr().max(1e-24);
        (y * x.conj()) / denom
    };

    let peak_norm = (1..x_spec.len())
        .map(|b| complex_transfer_at_bin(b).norm())
        .fold(1e-12_f64, f64::max);
    let coherent_floor = peak_norm * 1e-3; // -60 dB relative to passband peak

    // Unwrap phase continuously across FFT bins where transfer magnitude is above stopband noise floor
    let mut unwrapped_phase_deg = vec![0.0f64; x_spec.len()];
    if x_spec.len() > 1 {
        let mut h_prev = complex_transfer_at_bin(1);
        let mut acc_rad = if h_prev.norm() >= coherent_floor {
            h_prev.arg()
        } else {
            0.0
        };
        unwrapped_phase_deg[1] = acc_rad * (180.0 / PI);
        for (bin, slot) in unwrapped_phase_deg.iter_mut().enumerate().skip(2) {
            let h_cur = complex_transfer_at_bin(bin);
            if h_cur.norm() >= coherent_floor && h_prev.norm() >= coherent_floor {
                let d_phi = (h_cur * h_prev.conj()).arg();
                acc_rad += d_phi;
            }
            *slot = acc_rad * (180.0 / PI);
            if h_cur.norm() >= coherent_floor {
                h_prev = h_cur;
            }
        }
    }

    let transfer_at_bin = |bin: usize| -> (f64, f64, f64) {
        let h = complex_transfer_at_bin(bin);
        let mag_db = linear_to_db(h.norm());
        let phase_deg = if bin > 0 && bin < unwrapped_phase_deg.len() {
            unwrapped_phase_deg[bin]
        } else {
            h.arg() * (180.0 / PI)
        };

        let b_lo = bin.saturating_sub(1);
        let b_hi = (bin + 1).min(x_spec.len() - 1);
        let group_delay_ms = if b_hi > b_lo && h.norm() >= coherent_floor {
            let h_lo = complex_transfer_at_bin(b_lo);
            let h_hi = complex_transfer_at_bin(b_hi);
            let d_phi = (h_hi * h_lo.conj()).arg();
            let d_omega = 2.0 * PI * ((b_hi - b_lo) as f64) * bin_hz;
            ((-d_phi / d_omega) * 1000.0).clamp(-50.0, 50.0)
        } else {
            0.0
        };

        (mag_db, phase_deg, group_delay_ms)
    };

    let (dc_gain_db, _, _) = transfer_at_bin(0);

    let mut points = Vec::new();
    let mut max_gain_db = -400.0f64;
    let mut min_gain_db = 400.0f64;
    let mut gain_1khz_db = 0.0;

    for &f in probe_freqs {
        if f >= sample_rate * 0.48 {
            continue;
        }
        let bin = ((f / bin_hz).round() as usize).clamp(1, x_spec.len() - 1);
        let (mag_db, phase_deg, group_delay_ms) = transfer_at_bin(bin);
        if (f - 1000.0).abs() < 1.0 {
            gain_1khz_db = mag_db;
        }
        max_gain_db = max_gain_db.max(mag_db);
        min_gain_db = min_gain_db.min(mag_db);
        points.push(BodePoint {
            freq_hz: (bin as f64) * bin_hz,
            magnitude_db: mag_db,
            phase_deg,
            group_delay_ms,
        });
    }

    // Search full bin range for -3 dB cutoff point relative to passband reference (low frequency bin 1)
    let (ref_db, _, _) = transfer_at_bin(1);
    let target_db = ref_db - 3.0103;
    let mut cutoff_minus_3db_hz = None;
    for bin in 2..x_spec.len().saturating_sub(1) {
        let (m0, _, _) = transfer_at_bin(bin - 1);
        let (m1, _, _) = transfer_at_bin(bin);
        if (m0 >= target_db && m1 <= target_db) || (m0 <= target_db && m1 >= target_db) {
            let frac = if (m1 - m0).abs() > 1e-9 {
                (target_db - m0) / (m1 - m0)
            } else {
                0.0
            };
            cutoff_minus_3db_hz = Some(((bin - 1) as f64 + frac.clamp(0.0, 1.0)) * bin_hz);
            break;
        }
    }

    BodeReport {
        points,
        dc_gain_db,
        gain_1khz_db,
        cutoff_minus_3db_hz,
        max_gain_db,
        min_gain_db,
    }
}

/// Compute a 31-band ISO 1/3-octave spectral envelope (`(freq_hz, magnitude_dbfs)`) from a steady-state
/// audio buffer (e.g. an instrument note sustain) by integrating band energy within `[f * 2^(-1/6), f * 2^(+1/6)]`.
pub fn compute_third_octave_envelope(samples: &[f32], sample_rate: f64) -> Vec<(f64, f64)> {
    let n = samples.len().max(64);
    let win = make_window(WindowType::BlackmanHarris, n);
    let coherent_gain: f64 = win.iter().sum::<f64>() / (n as f64);

    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(n);
    let mut input: Vec<f64> = samples
        .iter()
        .zip(win.iter())
        .map(|(&s, &w)| (s as f64) * w)
        .collect();
    let mut spec = r2c.make_output_vec();
    let _ = r2c.process(&mut input, &mut spec);

    let norm = 2.0 / ((n as f64) * coherent_gain.max(1e-12));
    let bin_hz = sample_rate / (n as f64);
    let half_band_ratio = 2.0f64.powf(1.0 / 6.0);

    let probe_freqs: &[f64] = &[
        20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
        500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0,
        6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
    ];

    let mut out = Vec::with_capacity(probe_freqs.len());
    for &f in probe_freqs {
        if f >= sample_rate * 0.48 {
            continue;
        }
        let f_lo = f / half_band_ratio;
        let f_hi = f * half_band_ratio;
        let b_lo = ((f_lo / bin_hz).floor() as usize).clamp(1, spec.len() - 1);
        let b_hi = ((f_hi / bin_hz).ceil() as usize).clamp(b_lo, spec.len() - 1);
        let mut peak_mag = 0.0f64;
        for c in &spec[b_lo..=b_hi] {
            let m = c.norm() * norm;
            if m > peak_mag {
                peak_mag = m;
            }
        }
        out.push((f, linear_to_db(peak_mag).clamp(-140.0, 40.0)));
    }
    out
}
