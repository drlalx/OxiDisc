use crate::cd_reader::{AlbumArtOption, CdInfo};
use crate::discid::CdToc;
use std::f64::consts::PI;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

pub const CDDA_BYTES_PER_SECTOR: usize = 2352;
pub const CDDA_SAMPLES_PER_SECTOR: usize = 588; // 2352 / 4 (16-bit stereo = 4 bytes per sample frame)
pub const CDDA_SAMPLE_RATE: u32 = 44100;
pub const CDDA_CHANNELS: u32 = 2;

/// Checks whether the given device path or system environment specifies mock/test CD mode.
pub fn is_mock_device(device: &str) -> bool {
    let trimmed = device.trim().to_lowercase();
    if trimmed == "mock"
        || trimmed == "test"
        || trimmed == "test_disc"
        || trimmed == "virtual"
        || trimmed.starts_with("mock:")
        || trimmed.starts_with("test:")
    {
        return true;
    }

    if std::env::var("OXIDISC_MOCK_CD")
        .map(|v| {
            let v = v.trim().to_lowercase();
            v == "1" || v == "true" || v == "yes" || v == "on"
        })
        .unwrap_or(false)
    {
        return true;
    }

    false
}

/// Returns a Table of Contents (TOC) for the mock/test CD.
/// Default test disc matches the official MusicBrainz test vector:
/// - 6 audio tracks
/// - Leadout offset: 95462
/// - Offsets: [150, 15363, 32314, 46592, 63414, 80489]
/// - MusicBrainz Disc ID: "49HHV7Eb8UKF3aQiNmu1GR8vKTY-"
/// - GnuDB Disc ID: "3404f606"
pub fn get_mock_toc(device: &str) -> Result<CdToc, String> {
    let trimmed = device.trim().to_lowercase();

    if trimmed == "mock:fast" || std::env::var("OXIDISC_MOCK_FAST").is_ok() {
        // Fast mode with smaller sector ranges for ultra-fast automated testing
        let mut toc = CdToc::new(1, 4, 3000)?;
        toc.set_track_offset(1, 150)?;
        toc.set_track_offset(2, 800)?;
        toc.set_track_offset(3, 1500)?;
        toc.set_track_offset(4, 2200)?;
        return Ok(toc);
    }

    let mut toc = CdToc::new(1, 6, 95462)?;
    toc.set_track_offset(1, 150)?;
    toc.set_track_offset(2, 15363)?;
    toc.set_track_offset(3, 32314)?;
    toc.set_track_offset(4, 46592)?;
    toc.set_track_offset(5, 63414)?;
    toc.set_track_offset(6, 80489)?;

    Ok(toc)
}

/// Returns offline mock metadata info for testing without any network connection.
pub fn get_mock_fallback_info(toc: &CdToc) -> CdInfo {
    let disc_id = toc.calculate_disc_id();
    let track_count = toc.track_count();

    let default_titles = [
        "Overture in C Major",
        "Harmonic Pulse",
        "Acoustic Reflection",
        "Binary Rhythm",
        "Spectral Waves",
        "Finale & Lead-Out",
    ];

    let tracks: Vec<String> = (0..track_count)
        .map(|i| {
            if i < default_titles.len() {
                default_titles[i].to_string()
            } else {
                format!("Mock Track {:02}", i + 1)
            }
        })
        .collect();

    CdInfo {
        title: "Synthetic Test Album".to_string(),
        artist: "OxiDisc Audio Lab".to_string(),
        tracks,
        disc_id,
        album_cover_url: None,
        album_art_options: vec![
            AlbumArtOption {
                key: "small".to_string(),
                label: "Small thumbnail (Mock)".to_string(),
                url: "https://coverartarchive.org/release/d3dc4be9-9749-4959-99e5-133d0cb467fe/front-250.jpg".to_string(),
            },
            AlbumArtOption {
                key: "large".to_string(),
                label: "Large thumbnail (Mock)".to_string(),
                url: "https://coverartarchive.org/release/d3dc4be9-9749-4959-99e5-133d0cb467fe/front-500.jpg".to_string(),
            },
        ],
        metadata_error: None,
        metadata_provider: Some("Mock CD (Offline Synthetic)".to_string()),
        toc: Some(toc.clone()),
    }
}

/// Generates 16-bit 44.1kHz stereo PCM audio samples for a specific duration.
/// Each track number uses a distinct harmonic pitch for audible verification.
pub fn generate_mock_track_samples(track_num: usize, duration_secs: u32) -> Vec<i16> {
    let total_samples = (duration_secs as usize) * (CDDA_SAMPLE_RATE as usize) * 2; // stereo: 2 channels
    let mut samples = Vec::with_capacity(total_samples);

    // Frequencies: A4=440Hz, C#5=554Hz, E5=659Hz, A5=880Hz, etc.
    let base_freq = match track_num {
        1 => 440.0,
        2 => 554.37,
        3 => 659.25,
        4 => 880.0,
        5 => 987.77,
        _ => 523.25,
    };

    let sample_rate = CDDA_SAMPLE_RATE as f64;
    let frames = total_samples / 2;

    for frame in 0..frames {
        let t = (frame as f64) / sample_rate;
        // Primary tone + subtle 2nd harmonic
        let left_val =
            (2.0 * PI * base_freq * t).sin() * 0.7 + (2.0 * PI * (base_freq * 2.0) * t).sin() * 0.2;
        let right_val = (2.0 * PI * (base_freq * 1.002) * t).sin() * 0.7
            + (2.0 * PI * (base_freq * 2.002) * t).sin() * 0.2;

        let left_i16 = (left_val * 24000.0).clamp(-32767.0, 32767.0) as i16;
        let right_i16 = (right_val * 24000.0).clamp(-32767.0, 32767.0) as i16;

        samples.push(left_i16);
        samples.push(right_i16);
    }

    samples
}

/// Rips synthetic audio sectors directly into a writer, updating progress and checking cancellation.
pub fn rip_sectors_mock<W: Write, F: FnMut(f64)>(
    start_sector: u32,
    end_sector: u32,
    cancel_flag: &AtomicBool,
    mut progress_callback: F,
    writer: &mut W,
) -> Result<(), String> {
    let total_sectors = end_sector.saturating_sub(start_sector);
    if total_sectors == 0 {
        return Err("Mock sector range has 0 sectors".to_string());
    }

    // Limit chunk size for responsive progress reporting
    let chunk_sectors = 100u32;
    let mut current_sector = start_sector;
    let sample_rate = CDDA_SAMPLE_RATE as f64;

    // Pitch derived from sector offset
    let base_freq = 440.0 + ((start_sector % 1000) as f64 * 0.1);

    while current_sector < end_sector {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Ripping cancelled by user".to_string());
        }

        let remaining = end_sector - current_sector;
        let count = remaining.min(chunk_sectors);

        let mut buffer = Vec::with_capacity((count as usize) * CDDA_BYTES_PER_SECTOR);

        for s in 0..count {
            let sector_index = (current_sector + s) - start_sector;
            let start_frame = (sector_index as usize) * CDDA_SAMPLES_PER_SECTOR;

            for frame_offset in 0..CDDA_SAMPLES_PER_SECTOR {
                let frame = start_frame + frame_offset;
                let t = (frame as f64) / sample_rate;

                let left_val = (2.0 * PI * base_freq * t).sin() * 0.6;
                let right_val = (2.0 * PI * (base_freq * 1.003) * t).sin() * 0.6;

                let left_i16 = (left_val * 24000.0).clamp(-32767.0, 32767.0) as i16;
                let right_i16 = (right_val * 24000.0).clamp(-32767.0, 32767.0) as i16;

                buffer.extend_from_slice(&left_i16.to_le_bytes());
                buffer.extend_from_slice(&right_i16.to_le_bytes());
            }
        }

        writer
            .write_all(&buffer)
            .map_err(|e| format!("Failed writing mock audio PCM data: {e}"))?;

        current_sector += count;
        let done_sectors = current_sector - start_sector;
        let fraction = (done_sectors as f64) / (total_sectors as f64);
        progress_callback(fraction.min(1.0));
    }

    Ok(())
}

/// Generates standalone test disc files (`test_disc.cue` and `test_disc.bin`) on disk
/// that can be mounted by virtual optical drive emulators.
pub fn generate_test_disc_files(
    output_dir: &std::path::Path,
    fast: bool,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create directory {:?}: {e}", output_dir))?;

    let bin_path = output_dir.join("test_disc.bin");
    let cue_path = output_dir.join("test_disc.cue");

    let toc = get_mock_toc(if fast { "mock:fast" } else { "mock" })?;
    let mut file = std::fs::File::create(&bin_path)
        .map_err(|e| format!("Failed to create BIN file {:?}: {e}", bin_path))?;

    let cancel = AtomicBool::new(false);
    let bin_file_name = bin_path
        .file_name()
        .map(|f| f.to_string_lossy())
        .unwrap_or_default();
    let mut cue_lines = vec![format!("FILE \"{bin_file_name}\" BINARY")];

    let track_names = [
        "Overture in C Major",
        "Harmonic Pulse",
        "Acoustic Reflection",
        "Binary Rhythm",
        "Spectral Waves",
        "Finale & Lead-Out",
    ];

    let mut current_sector = 0u32;

    for t in toc.first_track..=toc.last_track {
        let track_idx = (t - 1) as usize;
        let start = toc.get_track_offset(t).unwrap_or(150);
        let end = if t < toc.last_track {
            toc.get_track_offset(t + 1).unwrap_or(start + 1000)
        } else {
            toc.leadout_offset
        };

        let track_sectors = if fast {
            200u32
        } else {
            end.saturating_sub(start).max(150)
        };

        rip_sectors_mock(start, start + track_sectors, &cancel, |_| {}, &mut file)
            .map_err(|e| format!("Failed writing track {t} PCM sectors: {e}"))?;

        let m = current_sector / (60 * 75);
        let s = (current_sector / 75) % 60;
        let f = current_sector % 75;

        let name = if track_idx < track_names.len() {
            track_names[track_idx]
        } else {
            "Audio Track"
        };

        cue_lines.push(format!("  TRACK {:02} AUDIO", t));
        cue_lines.push(format!("    TITLE \"{}\"", name));
        cue_lines.push("    PERFORMER \"OxiDisc Audio Lab\"".to_string());
        cue_lines.push(format!("    INDEX 01 {:02}:{:02}:{:02}", m, s, f));

        current_sector += track_sectors;
    }

    file.flush()
        .map_err(|e| format!("Failed flushing BIN file: {e}"))?;

    let cue_content = cue_lines.join("\r\n");
    std::fs::write(&cue_path, cue_content).map_err(|e| format!("Failed writing CUE file: {e}"))?;

    Ok((cue_path, bin_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mock_device() {
        assert!(is_mock_device("mock"));
        assert!(is_mock_device("MOCK"));
        assert!(is_mock_device("test"));
        assert!(is_mock_device("test_disc"));
        assert!(is_mock_device("mock:fast"));
        assert!(!is_mock_device("/dev/sr0"));
        assert!(!is_mock_device("D:"));
    }

    #[test]
    fn test_get_mock_toc() {
        let toc = get_mock_toc("mock").unwrap();
        assert_eq!(toc.track_count(), 6);
        assert_eq!(toc.calculate_disc_id(), "49HHV7Eb8UKF3aQiNmu1GR8vKTY-");
        assert_eq!(toc.cddb_disc_id_string(), "3404f606");
    }

    #[test]
    fn test_mock_fallback_info() {
        let toc = get_mock_toc("mock").unwrap();
        let info = get_mock_fallback_info(&toc);
        assert_eq!(info.title, "Synthetic Test Album");
        assert_eq!(info.tracks.len(), 6);
        assert_eq!(info.disc_id, "49HHV7Eb8UKF3aQiNmu1GR8vKTY-");
    }

    #[test]
    fn test_generate_mock_track_samples() {
        let samples = generate_mock_track_samples(1, 2);
        // 2 seconds * 44100 * 2 channels = 176400 samples
        assert_eq!(samples.len(), 176400);
    }

    #[test]
    fn test_rip_sectors_mock() {
        let cancel = AtomicBool::new(false);
        let mut output = Vec::new();
        let mut progress_values = Vec::new();

        let res = rip_sectors_mock(150, 250, &cancel, |p| progress_values.push(p), &mut output);
        assert!(res.is_ok());
        // 100 sectors * 2352 bytes = 235200 bytes
        assert_eq!(output.len(), 235200);
        assert!(!progress_values.is_empty());
        assert_eq!(*progress_values.last().unwrap(), 1.0);
    }
}
