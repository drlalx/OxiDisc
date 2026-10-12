use crate::discid::CdToc;

/// Constants for standard RedBook CDDA audio (44.1 kHz, 16-bit, Stereo)
pub const CDDA_SAMPLE_RATE: u32 = 44_100;
pub const CDDA_CHANNELS: u16 = 2;
pub const CDDA_BITS_PER_SAMPLE: u16 = 16;
pub const CDDA_BYTES_PER_SAMPLE: u16 = 2;
pub const CDDA_BLOCK_ALIGN: u16 = CDDA_CHANNELS * CDDA_BYTES_PER_SAMPLE; // 4
pub const CDDA_BYTE_RATE: u32 = CDDA_SAMPLE_RATE * (CDDA_BLOCK_ALIGN as u32); // 176,400
pub const CDDA_BYTES_PER_SECTOR: u32 = 2352;
pub const WAV_HEADER_SIZE: usize = 44;

/// Generates a standard 44-byte canonical RIFF/WAVE header for 44.1kHz 16-bit stereo PCM audio.
pub fn create_wav_header(pcm_data_len: u32) -> [u8; WAV_HEADER_SIZE] {
    let mut header = [0u8; WAV_HEADER_SIZE];

    // RIFF chunk descriptor
    header[0..4].copy_from_slice(b"RIFF");
    let riff_chunk_size = pcm_data_len.saturating_add(36);
    header[4..8].copy_from_slice(&riff_chunk_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");

    // "fmt " sub-chunk
    header[12..16].copy_from_slice(b"fmt ");
    let subchunk1_size = 16u32; // PCM chunk size
    header[16..20].copy_from_slice(&subchunk1_size.to_le_bytes());
    let audio_format = 1u16; // PCM = 1
    header[20..22].copy_from_slice(&audio_format.to_le_bytes());
    header[22..24].copy_from_slice(&CDDA_CHANNELS.to_le_bytes());
    header[24..28].copy_from_slice(&CDDA_SAMPLE_RATE.to_le_bytes());
    header[28..32].copy_from_slice(&CDDA_BYTE_RATE.to_le_bytes());
    header[32..34].copy_from_slice(&CDDA_BLOCK_ALIGN.to_le_bytes());
    header[34..36].copy_from_slice(&CDDA_BITS_PER_SAMPLE.to_le_bytes());

    // "data" sub-chunk
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&pcm_data_len.to_le_bytes());

    header
}

/// Calculates the start sector (inclusive) and end sector (exclusive) for a given track number.
pub fn track_sector_range(toc: &CdToc, track_num: usize) -> Result<(u32, u32), String> {
    if track_num < (toc.first_track as usize) || track_num > (toc.last_track as usize) {
        return Err(format!(
            "Track {track_num} is out of range {}..={}",
            toc.first_track, toc.last_track
        ));
    }

    let start_sector = toc
        .get_track_offset(track_num as u8)
        .ok_or_else(|| format!("Could not get offset for track {track_num}"))?;

    let end_sector = if track_num < (toc.last_track as usize) {
        toc.get_track_offset((track_num + 1) as u8)
            .ok_or_else(|| format!("Could not get offset for track {}", track_num + 1))?
    } else {
        toc.leadout_offset
    };

    if end_sector < start_sector {
        return Err(format!(
            "Track {track_num} end sector ({end_sector}) is before start sector ({start_sector})"
        ));
    }

    Ok((start_sector, end_sector))
}

/// Calculates the total PCM audio bytes for a track from start to end sector.
pub fn track_pcm_bytes(start_sector: u32, end_sector: u32) -> u64 {
    let sector_count = end_sector.saturating_sub(start_sector) as u64;
    sector_count * (CDDA_BYTES_PER_SECTOR as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header_structure() {
        let pcm_len = 176_400u32; // Exactly 1 second of audio
        let header = create_wav_header(pcm_len);

        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes(header[4..8].try_into().unwrap()),
            pcm_len + 36
        );
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(header[16..20].try_into().unwrap()), 16);
        assert_eq!(
            u16::from_le_bytes(header[20..22].try_into().unwrap()),
            1 // PCM format
        );
        assert_eq!(
            u16::from_le_bytes(header[22..24].try_into().unwrap()),
            2 // Stereo
        );
        assert_eq!(
            u32::from_le_bytes(header[24..28].try_into().unwrap()),
            44_100 // Sample rate
        );
        assert_eq!(
            u32::from_le_bytes(header[28..32].try_into().unwrap()),
            176_400 // Byte rate
        );
        assert_eq!(
            u16::from_le_bytes(header[32..34].try_into().unwrap()),
            4 // Block align
        );
        assert_eq!(
            u16::from_le_bytes(header[34..36].try_into().unwrap()),
            16 // Bits per sample
        );
        assert_eq!(&header[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes(header[40..44].try_into().unwrap()),
            pcm_len
        );
    }

    #[test]
    fn test_track_sector_range_calculation() {
        let mut toc = CdToc::new(1, 3, 50_000).unwrap();
        toc.set_track_offset(1, 150).unwrap();
        toc.set_track_offset(2, 10_000).unwrap();
        toc.set_track_offset(3, 25_000).unwrap();

        assert_eq!(track_sector_range(&toc, 1), Ok((150, 10_000)));
        assert_eq!(track_sector_range(&toc, 2), Ok((10_000, 25_000)));
        assert_eq!(track_sector_range(&toc, 3), Ok((25_000, 50_000)));
        assert!(track_sector_range(&toc, 0).is_err());
        assert!(track_sector_range(&toc, 4).is_err());

        assert_eq!(track_pcm_bytes(150, 10_000), (10_000 - 150) * 2352);
    }
}
