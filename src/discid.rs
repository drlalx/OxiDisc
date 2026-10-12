use sha1::{Digest, Sha1};
use std::fmt::Write;

/// Custom Base64 table used by MusicBrainz (`+` -> `.`, `/` -> `_`, `=` -> `-`)
const MUSICBRAINZ_BASE64_TABLE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._";

/// Representation of an Audio CD's Table of Contents (TOC)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdToc {
    pub first_track: u8,
    pub last_track: u8,
    pub leadout_offset: u32,
    /// 1-based track offsets (index 0 is unused or stores leadout).
    /// Tracks are indexed from 1 to 99.
    pub track_offsets: [u32; 100],
}

impl CdToc {
    /// Creates a new TOC with validation. Returns an error if the track range is invalid.
    pub fn new(first_track: u8, last_track: u8, leadout_offset: u32) -> Result<Self, String> {
        if first_track == 0 {
            return Err("First track cannot be 0".to_string());
        }
        if last_track < first_track {
            return Err(format!(
                "Last track ({last_track}) cannot be less than first track ({first_track})"
            ));
        }
        if last_track > 99 {
            return Err(format!("Last track ({last_track}) cannot exceed 99"));
        }

        Ok(Self {
            first_track,
            last_track,
            leadout_offset,
            track_offsets: [0; 100],
        })
    }

    /// Sets the offset (in sectors / frames) for a given track number (1..=99).
    pub fn set_track_offset(&mut self, track: u8, offset: u32) -> Result<(), String> {
        if track < 1 || track > 99 {
            return Err(format!("Track number {track} out of range 1..=99"));
        }
        self.track_offsets[track as usize] = offset;
        Ok(())
    }

    /// Returns the sector offset for a given track number (1..=last_track).
    pub fn get_track_offset(&self, track: u8) -> Option<u32> {
        if track < self.first_track || track > self.last_track {
            None
        } else {
            Some(self.track_offsets[track as usize])
        }
    }

    /// Returns the number of audio tracks.
    pub fn track_count(&self) -> usize {
        if self.last_track >= self.first_track {
            (self.last_track - self.first_track + 1) as usize
        } else {
            0
        }
    }

    /// Calculates the MusicBrainz Disc ID string (28 characters, URL-safe Base64).
    pub fn calculate_disc_id(&self) -> String {
        let mut hasher = Sha1::new();

        let mut hex_data = String::with_capacity(804);
        let _ = write!(&mut hex_data, "{:02X}", self.first_track);
        let _ = write!(&mut hex_data, "{:02X}", self.last_track);
        let _ = write!(&mut hex_data, "{:08X}", self.leadout_offset);

        for i in 1..=99 {
            let offset = self.track_offsets[i];
            let _ = write!(&mut hex_data, "{:08X}", offset);
        }

        hasher.update(hex_data.as_bytes());
        let digest = hasher.finalize();
        encode_musicbrainz_base64(&digest)
    }

    /// Formats the TOC string used for MusicBrainz Web Service queries:
    /// `<first_track> <last_track> <leadout_offset> <track1_offset> <track2_offset> ...`
    pub fn toc_string(&self) -> String {
        let mut result = format!(
            "{} {} {}",
            self.first_track, self.last_track, self.leadout_offset
        );
        for t in self.first_track..=self.last_track {
            let offset = self.track_offsets[t as usize];
            let _ = write!(result, " {}", offset);
        }
        result
    }

    /// Calculates the 32-bit CDDB / FreeDB / GnuDB Disc ID.
    pub fn calculate_cddb_disc_id(&self) -> u32 {
        let mut sum: u32 = 0;
        for t in self.first_track..=self.last_track {
            let offset_frames = self.track_offsets[t as usize];
            let offset_seconds = offset_frames / 75;
            sum = sum.wrapping_add(cddb_sum(offset_seconds));
        }

        let first_offset_frames = self.track_offsets[self.first_track as usize];
        let total_seconds = (self.leadout_offset / 75).saturating_sub(first_offset_frames / 75);
        let track_count = self.track_count() as u32;

        ((sum % 0xFF) << 24) | ((total_seconds & 0xFFFF) << 8) | (track_count & 0xFF)
    }

    /// Formats the CDDB Disc ID as an 8-character lowercase hex string.
    pub fn cddb_disc_id_string(&self) -> String {
        format!("{:08x}", self.calculate_cddb_disc_id())
    }

    /// Formats the parameters for a CDDB / GnuDB query command:
    /// `<discid>+<num_tracks>+<offset1>+<offset2>...+<leadout_seconds>`
    pub fn cddb_query_params(&self) -> String {
        let discid = self.cddb_disc_id_string();
        let track_count = self.track_count();
        let mut cmd = format!("{}+{}", discid, track_count);
        for t in self.first_track..=self.last_track {
            let offset = self.track_offsets[t as usize];
            let _ = write!(cmd, "+{}", offset);
        }
        let total_seconds = self.leadout_offset / 75;
        let _ = write!(cmd, "+{}", total_seconds);
        cmd
    }
}

/// Helper to calculate the sum of decimal digits of an integer.
fn cddb_sum(mut n: u32) -> u32 {
    let mut sum = 0;
    while n > 0 {
        sum += n % 10;
        n /= 10;
    }
    sum
}

/// Encodes 20-byte SHA-1 digest into a 28-character MusicBrainz URL-safe base64 string.
fn encode_musicbrainz_base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(28);
    let mut i = 0;

    while i < data.len() {
        let b0 = data.get(i).copied().unwrap_or(0);
        let b1 = data.get(i + 1).copied().unwrap_or(0);
        let b2 = data.get(i + 2).copied().unwrap_or(0);

        let remaining = data.len().saturating_sub(i);

        let idx0 = (b0 >> 2) & 0x3F;
        let idx1 = ((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F);
        let idx2 = ((b1 & 0x0F) << 2) | ((b2 >> 6) & 0x03);
        let idx3 = b2 & 0x3F;

        out.push(MUSICBRAINZ_BASE64_TABLE[idx0 as usize] as char);
        out.push(MUSICBRAINZ_BASE64_TABLE[idx1 as usize] as char);

        if remaining > 1 {
            out.push(MUSICBRAINZ_BASE64_TABLE[idx2 as usize] as char);
        } else {
            out.push('-');
        }

        if remaining > 2 {
            out.push(MUSICBRAINZ_BASE64_TABLE[idx3 as usize] as char);
        } else {
            out.push('-');
        }

        i += 3;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_official_musicbrainz_disc_id_standard_audio_cd() {
        // Official MusicBrainz test vector from documentation:
        // https://musicbrainz.org/doc/Disc_ID_Calculation
        // 6 tracks, leadout = 95462
        // Expected Disc ID: 49HHV7Eb8UKF3aQiNmu1GR8vKTY-
        let mut toc = CdToc::new(1, 6, 95462).unwrap();
        toc.set_track_offset(1, 150).unwrap();
        toc.set_track_offset(2, 15363).unwrap();
        toc.set_track_offset(3, 32314).unwrap();
        toc.set_track_offset(4, 46592).unwrap();
        toc.set_track_offset(5, 63414).unwrap();
        toc.set_track_offset(6, 80489).unwrap();

        assert_eq!(toc.track_count(), 6);
        assert_eq!(toc.calculate_disc_id(), "49HHV7Eb8UKF3aQiNmu1GR8vKTY-");
        assert_eq!(
            toc.toc_string(),
            "1 6 95462 150 15363 32314 46592 63414 80489"
        );
    }

    #[test]
    fn test_official_musicbrainz_disc_id_cd_extra_multisession() {
        // Official MusicBrainz test vector for CD-Extra multi-session:
        // 7 audio tracks, leadout = 114574
        // Expected Disc ID: BPnh1KU.hea1C.KMYWLGZkHJr0w-
        let mut toc = CdToc::new(1, 7, 114574).unwrap();
        toc.set_track_offset(1, 150).unwrap();
        toc.set_track_offset(2, 14109).unwrap();
        toc.set_track_offset(3, 33586).unwrap();
        toc.set_track_offset(4, 53077).unwrap();
        toc.set_track_offset(5, 65781).unwrap();
        toc.set_track_offset(6, 77892).unwrap();
        toc.set_track_offset(7, 99174).unwrap();

        assert_eq!(toc.track_count(), 7);
        assert_eq!(toc.calculate_disc_id(), "BPnh1KU.hea1C.KMYWLGZkHJr0w-");
        assert_eq!(
            toc.toc_string(),
            "1 7 114574 150 14109 33586 53077 65781 77892 99174"
        );
    }

    #[test]
    fn test_invalid_toc_creation() {
        assert!(CdToc::new(0, 5, 1000).is_err());
        assert!(CdToc::new(5, 2, 1000).is_err());
        assert!(CdToc::new(1, 100, 1000).is_err());
    }

    #[test]
    fn test_track_offset_out_of_bounds() {
        let mut toc = CdToc::new(1, 10, 50000).unwrap();
        assert!(toc.set_track_offset(0, 150).is_err());
        assert!(toc.set_track_offset(100, 150).is_err());
        assert!(toc.set_track_offset(1, 150).is_ok());
        assert_eq!(toc.get_track_offset(1), Some(150));
        assert_eq!(toc.get_track_offset(11), None);
    }

    #[test]
    fn test_cddb_disc_id_calculation() {
        let mut toc = CdToc::new(1, 6, 95462).unwrap();
        toc.set_track_offset(1, 150).unwrap();
        toc.set_track_offset(2, 15363).unwrap();
        toc.set_track_offset(3, 32314).unwrap();
        toc.set_track_offset(4, 46592).unwrap();
        toc.set_track_offset(5, 63414).unwrap();
        toc.set_track_offset(6, 80489).unwrap();

        assert_eq!(toc.calculate_cddb_disc_id(), 0x3404f606);
        assert_eq!(toc.cddb_disc_id_string(), "3404f606");
        assert_eq!(
            toc.cddb_query_params(),
            "3404f606+6+150+15363+32314+46592+63414+80489+1272"
        );
    }
}
