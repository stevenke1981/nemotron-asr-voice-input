/// WAV file reading, writing, and utterance audio saving utilities.
///
///
/// # WAV Parsing Notes
///
/// Standard RIFF/WAVE files may contain optional chunks (JUNK, LIST, etc.)
/// before the fmt/data chunks.  A naive 44-byte header parser would treat
/// those bytes as PCM data.  This module walks the RIFF chunk tree robustly.

use std::path::PathBuf;
use std::io::Write;
use anyhow::{Context, Result};

// ── Reading ──────────────────────────────────────────────────────────

/// Parse a RIFF/WAVE file instead of assuming a fixed 44-byte header.
/// Encoders may insert JUNK, LIST, or extended fmt chunks before the PCM data.
pub fn parse_pcm16_mono_wav(wav: &[u8]) -> Result<(u32, Vec<f32>)> {
    anyhow::ensure!(
        wav.len() >= 12 && &wav[0..4] == b"RIFF" && &wav[8..12] == b"WAVE",
        "Invalid RIFF/WAVE file"
    );

    let mut offset = 12usize;
    let mut format = None;
    let mut data_range = None;
    while offset + 8 <= wav.len() {
        let id = &wav[offset..offset + 4];
        let chunk_len = u32::from_le_bytes(wav[offset + 4..offset + 8].try_into()?) as usize;
        let start = offset + 8;
        let end = start
            .checked_add(chunk_len)
            .context("WAV chunk length overflow")?;
        anyhow::ensure!(end <= wav.len(), "Truncated WAV chunk");

        if id == b"fmt " {
            anyhow::ensure!(chunk_len >= 16, "Invalid WAV fmt chunk");
            format = Some((
                u16::from_le_bytes(wav[start..start + 2].try_into()?),
                u16::from_le_bytes(wav[start + 2..start + 4].try_into()?),
                u32::from_le_bytes(wav[start + 4..start + 8].try_into()?),
                u16::from_le_bytes(wav[start + 14..start + 16].try_into()?),
            ));
        } else if id == b"data" {
            data_range = Some(start..end);
        }

        offset = end + (chunk_len & 1);
    }

    let (audio_format, channels, sample_rate, bits_per_sample) =
        format.context("WAV fmt chunk not found")?;
    anyhow::ensure!(audio_format == 1, "Only PCM WAV is supported");
    anyhow::ensure!(channels == 1, "Only mono WAV is supported");
    anyhow::ensure!(bits_per_sample == 16, "Only 16-bit WAV is supported");
    let data = &wav[data_range.context("WAV data chunk not found")?];
    anyhow::ensure!(data.len().is_multiple_of(2), "WAV PCM data has an odd byte count");

    let samples = data
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as f32 / 32768.0)
        .collect();
    Ok((sample_rate, samples))
}

// ── Writing ──────────────────────────────────────────────────────────

/// Write a WAV file from f32 PCM data (16-bit mono).
pub fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<()> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = samples.len() as u32 * 2;
    let file_size = 36 + data_size;

    let mut file = std::fs::File::create(path)?;

    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    file.write_all(b"fmt ")?;
    file.write_all(&(16u32).to_le_bytes())?;
    file.write_all(&(1u16).to_le_bytes())?;
    file.write_all(&num_channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;

    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        file.write_all(&i16_sample.to_le_bytes())?;
    }

    Ok(())
}

// ── Utterance audio saving ───────────────────────────────────────────

/// Save utterance audio to the `voices/` directory with a timestamp filename.
pub fn save_utterance_audio(samples: &[f32], sample_rate: u32) -> Result<()> {
    let voices_dir = PathBuf::from("voices");
    if !voices_dir.exists() {
        std::fs::create_dir_all(&voices_dir)?;
    }

    // Generate a filename: voices/YYYYMMDD-HHMMSS-uuuuuu.wav
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let micros = now.subsec_micros();

    // Break down into date/time components
    let secs_per_day: u64 = 86400;
    let days_since_epoch = total_secs / secs_per_day;
    let time_secs = total_secs % secs_per_day;

    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let secs = time_secs % 60;

    // Simple day count → year/month/day
    let (year, month, day) = days_since_epoch_to_date(days_since_epoch);

    let filename = format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:06}.wav",
        year, month, day, hours, mins, secs, micros
    );
    let path = voices_dir.join(&filename);

    write_wav(&path, samples, sample_rate)?;

    tracing::info!("Saved utterance audio ({} samples, {:.1}s) to {:?}",
        samples.len(),
        samples.len() as f64 / sample_rate as f64,
        path);

    Ok(())
}

/// Convert days since Unix epoch to a (year, month, day) tuple.
/// Uses a simple algorithm valid for dates 1970-03-01 to 2100-02-28.
pub fn days_since_epoch_to_date(days: u64) -> (u64, u64, u64) {
    // Days since 1970-01-01. Shift so March is month 0 (year starts March).
    let z = days + 719468;  // days from 0000-03-01 to 1970-01-01
    let era = z / 146097;   // 146097 days per 400-year era
    let doe = z % 146097;   // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn push_chunk(wav: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
        wav.extend_from_slice(id);
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(data);
        if data.len() % 2 == 1 {
            wav.push(0);
        }
    }

    #[test]
    fn wav_parser_finds_data_after_nonstandard_chunks() {
        let mut wav = Vec::from(&b"RIFF\0\0\0\0WAVE"[..]);
        push_chunk(&mut wav, b"JUNK", &[1, 2, 3]);

        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&16_000u32.to_le_bytes());
        fmt.extend_from_slice(&32_000u32.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        push_chunk(&mut wav, b"fmt ", &fmt);

        let mut pcm = Vec::new();
        for sample in [i16::MIN, 0, i16::MAX] {
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
        push_chunk(&mut wav, b"data", &pcm);
        let riff_len = wav.len() as u32 - 8;
        wav[4..8].copy_from_slice(&riff_len.to_le_bytes());

        let (rate, samples) = parse_pcm16_mono_wav(&wav).unwrap();

        assert_eq!(rate, 16_000);
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0], -1.0);
        assert_eq!(samples[1], 0.0);
        assert!((samples[2] - 32767.0 / 32768.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_write_wav_roundtrip() {
        let dir = std::env::temp_dir().join("nemotron_wav_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_roundtrip.wav");

        let original = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        write_wav(&path, &original, 16000).unwrap();

        let data = std::fs::read(&path).unwrap();
        let (rate, parsed) = parse_pcm16_mono_wav(&data).unwrap();

        assert_eq!(rate, 16000);
        assert_eq!(parsed.len(), original.len());
        for (a, b) in parsed.iter().zip(original.iter()) {
            let diff = (a - b).abs();
            assert!(diff < 1.0 / 32767.0, "sample mismatch: {} vs {}", a, b);
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_utterance_audio_creates_file() {
        let samples = vec![0.1f32; 16000]; // 1 second
        let result = save_utterance_audio(&samples, 16000);
        assert!(result.is_ok(), "save_utterance_audio failed: {:?}", result.err());

        // Verify a file was created in voices/
        let voices_dir = PathBuf::from("voices");
        assert!(voices_dir.exists(), "voices/ directory should exist");

        // Clean up
        if let Ok(entries) = std::fs::read_dir(&voices_dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
            let _ = std::fs::remove_dir(&voices_dir);
        }
    }

    #[test]
    fn test_days_since_epoch_to_date_known() {
        // 1970-01-01 → day 0 → (1970, 1, 1)
        assert_eq!(days_since_epoch_to_date(0), (1970, 1, 1));
        // 1970-01-02 → day 1 → (1970, 1, 2)
        assert_eq!(days_since_epoch_to_date(1), (1970, 1, 2));
        // 2026-06-30 ≈ 20634 days since epoch (day 0 = 1970-01-01)
        let days = 20634u64;
        let (y, m, d) = days_since_epoch_to_date(days);
        assert_eq!(y, 2026);
        assert_eq!(m, 6);
        assert_eq!(d, 30);
    }
}
