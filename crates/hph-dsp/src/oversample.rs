use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

const NUM_PHASES: usize = 4;
const TAPS_PER_PHASE: usize = 16;
const HISTORY_MASK: usize = TAPS_PER_PHASE - 1;

#[inline]
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let half_x = x * 0.5;
    for k in 1..25 {
        let f = half_x / (k as f64);
        term *= f * f;
        sum += term;
        if term < sum * 1e-15 {
            break;
        }
    }
    sum
}

/// Build a 4-phase Kaiser-windowed sinc polyphase filter bank (64 total taps, 16 taps per phase).
/// Phase 0 is exact pass-through at center tap, Phases 1, 2, 3 reconstruct +0.25, +0.50, +0.75
/// fractional sample offsets to catch inter-sample True-Peaks and compute 4x oversampled RMS.
fn build_polyphase_4x_table() -> [[f64; TAPS_PER_PHASE]; NUM_PHASES] {
    let mut table = [[0.0f64; TAPS_PER_PHASE]; NUM_PHASES];
    let beta = 7.85; // >80 dB stopband attenuation
    let denom = bessel_i0(beta);
    let center = (TAPS_PER_PHASE as f64) * 0.5 - 1.0; // tap 7.0

    for (p, phase_coeffs) in table.iter_mut().enumerate() {
        let frac = (p as f64) / (NUM_PHASES as f64); // 0.0, 0.25, 0.50, 0.75
        let mut sum = 0.0;
        for (k, coeff) in phase_coeffs.iter_mut().enumerate() {
            let x = (k as f64) - center - frac;
            let sinc = if x.abs() < 1e-12 {
                1.0
            } else {
                (PI * x).sin() / (PI * x)
            };
            let norm =
                (2.0 * (k as f64 - frac) / (TAPS_PER_PHASE as f64 - 1.0) - 1.0).clamp(-1.0, 1.0);
            let win = bessel_i0(beta * (1.0 - norm * norm).max(0.0).sqrt()) / denom;
            let val = sinc * win;
            *coeff = val;
            sum += val;
        }
        // Normalize DC gain of each phase to 1.0000000
        for coeff in phase_coeffs.iter_mut() {
            *coeff /= sum;
        }
    }
    table
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeterReport {
    pub sample_peak_linear: f64,
    pub sample_peak_dbfs: f64,
    pub true_peak_4x_linear: f64,
    pub true_peak_4x_dbtp: f64,
    pub intersample_overshoot_db: f64,
    pub rms_sample_linear: f64,
    pub rms_sample_dbfs: f64,
    pub rms_4x_linear: f64,
    pub rms_4x_dbfs: f64,
    pub k_weighted_lufs: f64,
    pub crest_factor_db: f64,
    pub dc_offset_linear: f64,
    pub dc_offset_dbfs: f64,
    pub nan_count: usize,
    pub inf_count: usize,
    pub subnormal_count: usize,
    pub clipped_samples_count: usize,
    pub max_slew_rate: f64,
}

#[inline]
pub fn linear_to_db(val: f64) -> f64 {
    if val <= 1e-20 {
        -400.0
    } else {
        20.0 * val.log10()
    }
}

pub struct OversampledMeter {
    coeffs: [[f64; TAPS_PER_PHASE]; NUM_PHASES],
    history: [f64; TAPS_PER_PHASE],
    write_pos: usize,
    sample_count: u64,
    sample_peak: f64,
    true_peak_4x: f64,
    sum_sq_1x: f64,
    sum_sq_4x: f64,
    sum_sq_k: f64,
    kw_s1_z1: f64,
    kw_s1_z2: f64,
    kw_s2_z1: f64,
    kw_s2_z2: f64,
    sum_dc: f64,
    nan_count: usize,
    inf_count: usize,
    subnormal_count: usize,
    clipped_samples_count: usize,
    prev_sample: Option<f64>,
    max_slew_rate: f64,
}

impl Default for OversampledMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl OversampledMeter {
    pub fn new() -> Self {
        Self {
            coeffs: build_polyphase_4x_table(),
            history: [0.0; TAPS_PER_PHASE],
            write_pos: 0,
            sample_count: 0,
            sample_peak: 0.0,
            true_peak_4x: 0.0,
            sum_sq_1x: 0.0,
            sum_sq_4x: 0.0,
            sum_sq_k: 0.0,
            kw_s1_z1: 0.0,
            kw_s1_z2: 0.0,
            kw_s2_z1: 0.0,
            kw_s2_z2: 0.0,
            sum_dc: 0.0,
            nan_count: 0,
            inf_count: 0,
            subnormal_count: 0,
            clipped_samples_count: 0,
            prev_sample: None,
            max_slew_rate: 0.0,
        }
    }

    pub fn process_slice(&mut self, samples: &[f32]) {
        for &s in samples {
            self.push_sample(s);
        }
    }

    #[inline]
    pub fn push_sample(&mut self, s: f32) {
        if s.is_nan() {
            self.nan_count += 1;
            return;
        }
        if s.is_infinite() {
            self.inf_count += 1;
            return;
        }
        if s.is_subnormal() {
            self.subnormal_count += 1;
        }

        let x = s as f64;
        let abs_x = x.abs();
        if abs_x >= 1.0 {
            self.clipped_samples_count += 1;
        }
        if abs_x > self.sample_peak {
            self.sample_peak = abs_x;
        }
        if let Some(prev) = self.prev_sample {
            let slew = (x - prev).abs();
            if slew > self.max_slew_rate {
                self.max_slew_rate = slew;
            }
        }
        self.prev_sample = Some(x);

        self.sample_count += 1;
        self.sum_dc += x;
        self.sum_sq_1x += x * x;

        // ITU-R BS.1770-4 K-Weighting Filter Cascade (Stage 1 High-Shelf + Stage 2 RLB High-Pass)
        let s1_y = 1.53512485958697 * x + self.kw_s1_z1;
        self.kw_s1_z1 = -2.69169618940638 * x - (-1.69065929318241) * s1_y + self.kw_s1_z2;
        self.kw_s1_z2 = 1.19839281085285 * x - 0.73248077421585 * s1_y;

        let s2_y = 1.0 * s1_y + self.kw_s2_z1;
        self.kw_s2_z1 = -2.0 * s1_y - (-1.99004745483398) * s2_y + self.kw_s2_z2;
        self.kw_s2_z2 = 1.0 * s1_y - 0.99007225036621 * s2_y;

        self.sum_sq_k += s2_y * s2_y;

        self.history[self.write_pos] = x;
        let newest = self.write_pos;
        self.write_pos = (self.write_pos + 1) & HISTORY_MASK;

        for phase in &self.coeffs {
            let mut acc = 0.0;
            for (k, &c) in phase.iter().enumerate() {
                let idx = newest.wrapping_sub(k) & HISTORY_MASK;
                acc += c * self.history[idx];
            }
            let abs_y = acc.abs();
            if abs_y > self.true_peak_4x {
                self.true_peak_4x = abs_y;
            }
            self.sum_sq_4x += acc * acc;
        }
    }

    /// Flush filter tail with zeros so peaks near the end of the buffer are measured.
    pub fn finalize(mut self) -> ChannelMeterReport {
        let total_real_samples = self.sample_count;
        for _ in 0..(TAPS_PER_PHASE / 2) {
            self.history[self.write_pos] = 0.0;
            let newest = self.write_pos;
            self.write_pos = (self.write_pos + 1) & HISTORY_MASK;
            for phase in &self.coeffs {
                let mut acc = 0.0;
                for (k, &c) in phase.iter().enumerate() {
                    let idx = newest.wrapping_sub(k) & HISTORY_MASK;
                    acc += c * self.history[idx];
                }
                let abs_y = acc.abs();
                if abs_y > self.true_peak_4x {
                    self.true_peak_4x = abs_y;
                }
            }
        }

        // True peak is at least the sample peak
        if self.sample_peak > self.true_peak_4x {
            self.true_peak_4x = self.sample_peak;
        }

        let n = total_real_samples.max(1) as f64;
        let rms_1x = (self.sum_sq_1x / n).sqrt();
        let rms_4x = (self.sum_sq_4x / (n * NUM_PHASES as f64)).sqrt();
        let mean_sq_k = self.sum_sq_k / n;
        let k_weighted_lufs = if mean_sq_k <= 1e-20 {
            -400.0
        } else {
            -0.691 + 10.0 * mean_sq_k.log10()
        };
        let dc = self.sum_dc / n;

        let sample_peak_dbfs = linear_to_db(self.sample_peak);
        let true_peak_4x_dbtp = linear_to_db(self.true_peak_4x);
        let rms_4x_dbfs = linear_to_db(rms_4x);

        ChannelMeterReport {
            sample_peak_linear: self.sample_peak,
            sample_peak_dbfs,
            true_peak_4x_linear: self.true_peak_4x,
            true_peak_4x_dbtp,
            intersample_overshoot_db: (true_peak_4x_dbtp - sample_peak_dbfs).max(0.0),
            rms_sample_linear: rms_1x,
            rms_sample_dbfs: linear_to_db(rms_1x),
            rms_4x_linear: rms_4x,
            rms_4x_dbfs,
            k_weighted_lufs,
            crest_factor_db: if rms_4x > 1e-20 {
                true_peak_4x_dbtp - rms_4x_dbfs
            } else {
                0.0
            },
            dc_offset_linear: dc,
            dc_offset_dbfs: linear_to_db(dc.abs()),
            nan_count: self.nan_count,
            inf_count: self.inf_count,
            subnormal_count: self.subnormal_count,
            clipped_samples_count: self.clipped_samples_count,
            max_slew_rate: self.max_slew_rate,
        }
    }
}

pub fn analyze_buffer_4x(samples: &[f32]) -> ChannelMeterReport {
    let mut meter = OversampledMeter::new();
    meter.process_slice(samples);
    meter.finalize()
}
