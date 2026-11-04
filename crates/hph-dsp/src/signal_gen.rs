use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalSpec {
    Silence,
    Sine {
        freq_hz: f64,
        amplitude: f64,
        phase_rad: f64,
    },
    TwoTone {
        freq1_hz: f64,
        amp1: f64,
        freq2_hz: f64,
        amp2: f64,
    },
    Saw {
        freq_hz: f64,
        amplitude: f64,
    },
    Square {
        freq_hz: f64,
        amplitude: f64,
        duty: f64,
    },
    Triangle {
        freq_hz: f64,
        amplitude: f64,
    },
    Impulse {
        offset_samples: usize,
        amplitude: f64,
    },
    TriggerTrain {
        freq_hz: f64,
        pulse_samples: usize,
        amplitude: f64,
    },
    WhiteNoise {
        amplitude: f64,
        seed: u64,
    },
    PinkNoise {
        amplitude: f64,
        seed: u64,
    },
    SubnormalNoise {
        seed: u64,
    },
    DcOffset {
        level: f64,
    },
    LogSweep {
        start_hz: f64,
        end_hz: f64,
        amplitude: f64,
    },
}

#[inline]
fn poly_blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Deterministic XorShift64* PRNG for repeatable noise & fuzzing.
#[derive(Debug, Clone)]
pub struct FastRng {
    state: u64,
}

impl FastRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x853c_49e6_748f_ea9b
            } else {
                seed
            },
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Returns uniform float in `[-1.0, 1.0]`.
    #[inline]
    pub fn next_bipolar(&mut self) -> f64 {
        let u = (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        u * 2.0 - 1.0
    }
}

pub fn generate_signal(spec: &SignalSpec, sample_rate: f64, num_frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; num_frames];
    match *spec {
        SignalSpec::Silence => {}
        SignalSpec::Sine {
            freq_hz,
            amplitude,
            phase_rad,
        } => {
            let w = 2.0 * PI * freq_hz / sample_rate;
            for (n, s) in out.iter_mut().enumerate() {
                *s = (amplitude * (w * n as f64 + phase_rad).sin()) as f32;
            }
        }
        SignalSpec::TwoTone {
            freq1_hz,
            amp1,
            freq2_hz,
            amp2,
        } => {
            let w1 = 2.0 * PI * freq1_hz / sample_rate;
            let w2 = 2.0 * PI * freq2_hz / sample_rate;
            for (n, s) in out.iter_mut().enumerate() {
                let t = n as f64;
                *s = (amp1 * (w1 * t).sin() + amp2 * (w2 * t).sin()) as f32;
            }
        }
        SignalSpec::Saw { freq_hz, amplitude } => {
            let dt = (freq_hz / sample_rate).clamp(0.0, 0.5);
            let mut phase = 0.0;
            for s in &mut out {
                let mut val = 2.0 * phase - 1.0;
                val -= poly_blep(phase, dt);
                *s = (amplitude * val) as f32;
                phase += dt;
                if phase >= 1.0 {
                    phase -= 1.0;
                }
            }
        }
        SignalSpec::Square {
            freq_hz,
            amplitude,
            duty,
        } => {
            let dt = (freq_hz / sample_rate).clamp(0.0, 0.5);
            let pw = duty.clamp(0.05, 0.95);
            let mut phase = 0.0;
            for s in &mut out {
                let mut val = if phase < pw { 1.0 } else { -1.0 };
                val += poly_blep(phase, dt);
                val -= poly_blep((phase + 1.0 - pw).rem_euclid(1.0), dt);
                *s = (amplitude * val) as f32;
                phase += dt;
                if phase >= 1.0 {
                    phase -= 1.0;
                }
            }
        }
        SignalSpec::Triangle { freq_hz, amplitude } => {
            let w = 2.0 * PI * freq_hz / sample_rate;
            for (n, s) in out.iter_mut().enumerate() {
                let v = (2.0 / PI) * (w * n as f64).sin().asin();
                *s = (amplitude * v) as f32;
            }
        }
        SignalSpec::Impulse {
            offset_samples,
            amplitude,
        } => {
            if offset_samples < num_frames {
                out[offset_samples] = amplitude as f32;
            }
        }
        SignalSpec::TriggerTrain {
            freq_hz,
            pulse_samples,
            amplitude,
        } => {
            let period = (sample_rate / freq_hz.max(0.01)).round().max(1.0) as usize;
            let width = pulse_samples.max(1);
            for (n, s) in out.iter_mut().enumerate() {
                if (n % period) < width {
                    *s = amplitude as f32;
                }
            }
        }
        SignalSpec::WhiteNoise { amplitude, seed } => {
            let mut rng = FastRng::new(seed);
            for s in &mut out {
                *s = (amplitude * rng.next_bipolar()) as f32;
            }
        }
        SignalSpec::PinkNoise { amplitude, seed } => {
            // Paul Kellet's refined 7-state pink noise filter
            let mut rng = FastRng::new(seed);
            let mut b0 = 0.0;
            let mut b1 = 0.0;
            let mut b2 = 0.0;
            let mut b3 = 0.0;
            let mut b4 = 0.0;
            let mut b5 = 0.0;
            let mut b6 = 0.0;
            for s in &mut out {
                let white = rng.next_bipolar();
                b0 = 0.99886 * b0 + white * 0.0555179;
                b1 = 0.99332 * b1 + white * 0.0750759;
                b2 = 0.96900 * b2 + white * 0.1538520;
                b3 = 0.86650 * b3 + white * 0.3104856;
                b4 = 0.55000 * b4 + white * 0.5329522;
                b5 = -0.7616 * b5 - white * 0.0168980;
                let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
                b6 = white * 0.115926;
                *s = (amplitude * pink * 0.15) as f32;
            }
        }
        SignalSpec::SubnormalNoise { seed } => {
            let mut rng = FastRng::new(seed);
            for s in &mut out {
                // IEEE-754 f32 subnormal: exponent = 0, mantissa != 0
                let mantissa = (rng.next_u64() as u32) & 0x007f_ffff;
                let sign = ((rng.next_u64() as u32) & 1) << 31;
                let bits = sign | mantissa.max(1);
                *s = f32::from_bits(bits);
            }
        }
        SignalSpec::DcOffset { level } => {
            out.fill(level as f32);
        }
        SignalSpec::LogSweep {
            start_hz,
            end_hz,
            amplitude,
        } => {
            let duration_sec = (num_frames as f64) / sample_rate;
            let f1 = start_hz.max(1.0);
            let f2 = end_hz.min(sample_rate * 0.499).max(f1 + 1.0);
            let ratio = (f2 / f1).ln();
            let k = 2.0 * PI * f1 * duration_sec / ratio;
            for (n, s) in out.iter_mut().enumerate() {
                let t = (n as f64) / sample_rate;
                let phase = k * ((t / duration_sec * ratio).exp() - 1.0);
                *s = (amplitude * phase.sin()) as f32;
            }
        }
    }
    out
}
