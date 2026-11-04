use anyhow::{Context, Result};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AudioFileData {
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
    pub num_frames: usize,
}

pub fn read_wav_file(path: &Path) -> Result<AudioFileData> {
    let mut reader =
        WavReader::open(path).with_context(|| format!("Failed to open WAV: {}", path.display()))?;
    let spec = reader.spec();
    let num_ch = (spec.channels as usize).max(1);

    let interleaved: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<f32>, _>>()?,
        SampleFormat::Int => {
            let max_val = ((1i64 << (spec.bits_per_sample.saturating_sub(1))) as f32).max(1.0);
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| (v as f32) / max_val))
                .collect::<std::result::Result<Vec<f32>, _>>()?
        }
    };

    let num_frames = interleaved.len() / num_ch;
    let mut channels = vec![Vec::with_capacity(num_frames); num_ch];
    for frame in 0..num_frames {
        for ch in 0..num_ch {
            channels[ch].push(interleaved[frame * num_ch + ch]);
        }
    }

    Ok(AudioFileData {
        sample_rate: spec.sample_rate,
        channels,
        num_frames,
    })
}

pub fn write_wav_file_f32(path: &Path, sample_rate: u32, channels: &[Vec<f32>]) -> Result<()> {
    let num_ch = channels.len().max(1);
    let num_frames = channels.first().map(|c| c.len()).unwrap_or(0);
    let spec = WavSpec {
        channels: num_ch as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer = WavWriter::create(path, spec)
        .with_context(|| format!("Failed to create WAV: {}", path.display()))?;

    for frame in 0..num_frames {
        for ch in channels {
            let s = ch.get(frame).copied().unwrap_or(0.0);
            writer.write_sample(s)?;
        }
    }
    writer.finalize()?;
    Ok(())
}
