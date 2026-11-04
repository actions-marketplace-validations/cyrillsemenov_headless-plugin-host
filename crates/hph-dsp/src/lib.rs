pub mod dynamics_stereo;
pub mod oversample;
pub mod signal_gen;
pub mod spectrum;
pub mod time_domain;
pub mod wav;

pub use dynamics_stereo::*;
pub use oversample::*;
pub use signal_gen::*;
pub use spectrum::*;
pub use time_domain::*;
pub use wav::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_4x_oversampled_intersample_true_peak() {
        // Construct the classic fs/4 cosine shifted by 45 degrees with a smooth Hann fade-in/out
        // so step onset doesn't cause Gibbs overshoot:
        // Samples at 1x hit +/- cos(pi/4) = +/- 0.70710678 (-3.01 dBFS),
        // while the true continuous analog peak between samples is 1.0000 (0.0 dBTP),
        // i.e., +3.01 dB inter-sample overshoot!
        let n = 512;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let env = (PI * (i as f64) / ((n - 1) as f64)).sin();
                (env * ((PI * 0.5 * (i as f64)) + (PI * 0.25)).cos()) as f32
            })
            .collect();

        let report = analyze_buffer_4x(&samples);
        assert!(
            (report.sample_peak_dbfs - (-3.0103)).abs() < 0.05,
            "Expected sample peak ~ -3.01 dBFS, got {}",
            report.sample_peak_dbfs
        );
        assert!(
            report.true_peak_4x_dbtp.abs() < 0.08,
            "Expected 4x True-Peak ~ 0.00 dBTP, got {}",
            report.true_peak_4x_dbtp
        );
        assert!(
            report.intersample_overshoot_db > 2.9,
            "Expected inter-sample overshoot > 2.9 dB, got {}",
            report.intersample_overshoot_db
        );
    }

    #[test]
    fn test_arbitrary_length_fft_and_thdn() {
        // 48000 samples (exact 1.0 Hz bin width at 48 kHz!)
        let sr = 48000.0;
        let n = 48000;
        let pure_1k = generate_signal(
            &SignalSpec::Sine {
                freq_hz: 1000.0,
                amplitude: 0.5,
                phase_rad: 0.0,
            },
            sr,
            n,
        );

        let spec = compute_spectrum(&pure_1k, sr, WindowType::BlackmanHarris);
        assert!((spec.peak_freq_hz - 1000.0).abs() < 0.5);

        let thdn = analyze_thdn(&pure_1k, sr, 1000.0);
        assert!(
            thdn.thd_percent < 0.001,
            "Pure sine THD should be < 0.001%, got {}",
            thdn.thd_percent
        );

        // Now apply cubic soft-clipping y = x - x^3 / 3 to introduce 3rd harmonic (3000 Hz)
        let clipped: Vec<f32> = pure_1k.iter().map(|&x| x - (x * x * x) / 3.0).collect();
        let thdn_clipped = analyze_thdn(&clipped, sr, 1000.0);
        assert!(
            thdn_clipped.thd_percent > 1.0,
            "Soft-clipped sine should have measurable THD > 1%, got {}",
            thdn_clipped.thd_percent
        );
        let h3 = thdn_clipped
            .harmonics
            .iter()
            .find(|h| h.order == 3)
            .unwrap();
        assert!(h3.level_dbc > -40.0);
    }

    #[test]
    fn test_bode_transfer_function_1pole_lowpass() {
        let sr = 48000.0;
        let n = 8192;
        let impulse = generate_signal(
            &SignalSpec::Impulse {
                offset_samples: 0,
                amplitude: 1.0,
            },
            sr,
            n,
        );

        // 1-pole lowpass filter with fc = 1000 Hz
        let fc = 1000.0;
        let alpha = 1.0 - (-2.0 * PI * fc / sr).exp();
        let mut y = 0.0f64;
        let output: Vec<f32> = impulse
            .iter()
            .map(|&x| {
                y += alpha * ((x as f64) - y);
                y as f32
            })
            .collect();

        let bode = compute_bode_response(&impulse, &output, sr);
        assert!(bode.dc_gain_db.abs() < 0.1);
        let measured_fc = bode.cutoff_minus_3db_hz.expect("Should find -3dB cutoff");
        assert!(
            (measured_fc - 1000.0).abs() < 80.0,
            "Expected -3dB cutoff near 1000 Hz, got {measured_fc}"
        );
    }

    #[test]
    fn test_schroeder_rt60_synthetic_reverb() {
        let sr = 48000.0;
        let duration_sec = 3.0;
        let n = (sr * duration_sec) as usize;
        let target_rt60 = 1.50; // 1.50 seconds to decay by 60 dB
                                // Amplitude decays as 10^(-3 * t / RT60) = exp(-ln(1000) * t / RT60)
        let decay_rate = (1000.0f64).ln() / target_rt60;
        let white = generate_signal(
            &SignalSpec::WhiteNoise {
                amplitude: 1.0,
                seed: 42,
            },
            sr,
            n,
        );
        let ir: Vec<f32> = white
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let t = (i as f64) / sr;
                (w as f64 * (-decay_rate * t).exp()) as f32
            })
            .collect();

        let rep = analyze_rt60(&ir, sr);
        assert!(
            (rep.rt60_sec - target_rt60).abs() < 0.05,
            "Expected RT60 ~ {target_rt60}s, got {:.4}s (T20={:.4}s, T30={:.4}s)",
            rep.rt60_sec,
            rep.t20_sec,
            rep.t30_sec
        );
        assert!(
            rep.r_squared > 0.99,
            "Expected linear decay R^2 > 0.99, got {:.4}",
            rep.r_squared
        );
    }

    #[test]
    fn test_delay_and_multitap_echo_finder() {
        let sr = 48000.0;
        let n = 4096;
        let input = generate_signal(
            &SignalSpec::Impulse {
                offset_samples: 0,
                amplitude: 1.0,
            },
            sr,
            n,
        );
        // 4 echo taps spaced by 240 samples (5.0 ms @ 48 kHz), decaying by -6.0206 dB (0.5x) per tap
        let mut output = vec![0.0f32; n];
        output[240] = 1.0;
        output[480] = 0.5;
        output[720] = 0.25;
        output[960] = 0.125;

        let rep = analyze_delay_and_latency(&input, &output, sr, Some(200));
        assert_eq!(rep.onset_delay_samples, 240);
        assert!((rep.peak_delay_ms - 5.0).abs() < 0.05);
        assert!((rep.xcorr_delay_samples - 240.0).abs() < 1.0);
        assert_eq!(rep.latency_discrepancy_samples, Some(40.0));
        assert_eq!(rep.echo_taps.len(), 4);
        assert!((rep.estimated_tap_spacing_ms.unwrap() - 5.0).abs() < 0.05);
        assert!((rep.estimated_feedback_db_per_tap.unwrap() - (-6.0206)).abs() < 0.1);
    }

    #[test]
    fn test_two_tone_imd_and_even_odd_thd() {
        let sr = 48000.0;
        let n = 48000;
        let two_tone = generate_signal(
            &SignalSpec::TwoTone {
                freq1_hz: 19000.0,
                amp1: 0.4,
                freq2_hz: 20000.0,
                amp2: 0.4,
            },
            sr,
            n,
        );
        // Asymmetric + symmetric waveshaper: y = x + 0.08 * x^2 - 0.12 * x^3
        // Generates 1 kHz difference tone (from x^2) and 18 kHz / 21 kHz IM3 tones (from x^3)
        let shaped: Vec<f32> = two_tone
            .iter()
            .map(|&s| {
                let x = s as f64;
                (x + 0.08 * x * x - 0.12 * x * x * x) as f32
            })
            .collect();

        let imd = analyze_imd(&shaped, sr, 19000.0, 20000.0);
        assert!(
            imd.imd_percent > 1.0,
            "Expected measurable IMD > 1.0%, got {:.4}%",
            imd.imd_percent
        );
        assert!(
            imd.diff_tone_dbc > -45.0,
            "Expected 1 kHz difference tone > -45 dBc, got {:.2} dBc",
            imd.diff_tone_dbc
        );
        assert!(
            imd.third_order_lower_dbc > -45.0,
            "Expected 18 kHz IMD3- tone > -45 dBc, got {:.2} dBc",
            imd.third_order_lower_dbc
        );

        // Also verify even vs odd harmonic balance on a 1 kHz tone through the same shaper
        let sine_1k = generate_signal(
            &SignalSpec::Sine {
                freq_hz: 1000.0,
                amplitude: 0.5,
                phase_rad: 0.0,
            },
            sr,
            n,
        );
        let shaped_1k: Vec<f32> = sine_1k
            .iter()
            .map(|&s| {
                let x = s as f64;
                (x + 0.08 * x * x - 0.12 * x * x * x) as f32
            })
            .collect();
        let thdn = analyze_thdn(&shaped_1k, sr, 1000.0);
        assert!(thdn.even_thd_db > -50.0 && thdn.odd_thd_db > -50.0);
    }

    #[test]
    fn test_stereo_image_and_k_weighted_lufs() {
        let sr = 48000.0;
        let n = 48000;
        let left = generate_signal(
            &SignalSpec::Sine {
                freq_hz: 1000.0,
                amplitude: 1.0,
                phase_rad: 0.0,
            },
            sr,
            n,
        );
        let right_inv: Vec<f32> = left.iter().map(|&x| -x).collect();
        let right_quad = generate_signal(
            &SignalSpec::Sine {
                freq_hz: 1000.0,
                amplitude: 1.0,
                phase_rad: PI * 0.5,
            },
            sr,
            n,
        );

        let mono_rep = analyze_stereo_image(&left, &left);
        assert!((mono_rep.phase_correlation - 1.0).abs() < 1e-5);
        assert!(mono_rep.stereo_width < 1e-5);

        let inv_rep = analyze_stereo_image(&left, &right_inv);
        assert!((inv_rep.phase_correlation - (-1.0)).abs() < 1e-5);
        assert!((inv_rep.stereo_width - 1.0).abs() < 1e-5);

        let quad_rep = analyze_stereo_image(&left, &right_quad);
        assert!(quad_rep.phase_correlation.abs() < 1e-3);

        // Full-scale 1 kHz sine has K-weighted loudness near -3.01 LUFS (single channel)
        let meter = analyze_buffer_4x(&left[4096..]);
        assert!(
            (meter.k_weighted_lufs - (-3.01)).abs() < 0.25,
            "Expected ~ -3.01 LUFS for 0 dBFS 1 kHz sine, got {:.2} LUFS",
            meter.k_weighted_lufs
        );
    }
}
