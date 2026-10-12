use crate::cd_reader::{AlbumArtOption, CdInfo};
use crate::discid::CdToc;
use chromaprint::{Algorithm, fingerprint_audio};
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_ACOUSTID_CLIENT_KEY: &str = "8XaBELgH";

#[derive(Debug, Clone, PartialEq)]
pub struct AcoustidMatch {
    pub recording_id: Option<String>,
    pub release_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub score: f64,
    pub track_count: Option<usize>,
}

pub struct AcoustidClient;

impl AcoustidClient {
    // ## Generates an AcoustID compressed Base64 fingerprint from interleaved 16-bit PCM audio samples.
    pub fn fingerprint_pcm16le(
        samples: &[i16],
        sample_rate: u32,
        channels: u32,
    ) -> Result<String, String> {
        if samples.is_empty() {
            return Err("PCM audio sample buffer is empty".to_string());
        }

        let result = fingerprint_audio(samples, sample_rate, channels as u16, Algorithm::default())
            .map_err(|err| format!("Chromaprint fingerprinting error: {err:?}"))?;

        Ok(result.encoded().to_string())
    }

    // ## Parses JSON response from AcoustID API lookup endpoint.
    pub fn parse_acoustid_response(response_body: &str) -> Result<AcoustidMatch, String> {
        let root: Value = serde_json::from_str(response_body)
            .map_err(|err| format!("Failed to parse AcoustID JSON response: {err}"))?;

        let status = root
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("error");

        if status != "ok" {
            let msg = root
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown AcoustID API error");
            return Err(format!("AcoustID returned error: {msg}"));
        }

        let results = root
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| "No results array in AcoustID response".to_string())?;

        if results.is_empty() {
            return Err("No matching acoustic fingerprint found in AcoustID database".to_string());
        }

        // Pick highest-scoring result with recording metadata
        for result in results {
            let score = result.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            if score < 0.4 {
                continue;
            }

            if let Some(recordings) = result.get("recordings").and_then(|r| r.as_array()) {
                if let Some(rec) = recordings.first() {
                    let recording_id = rec
                        .get("id")
                        .and_then(|i| i.as_str())
                        .map(|s| s.to_string());
                    let title = rec
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Unknown Track")
                        .to_string();

                    let artist = rec
                        .get("artists")
                        .and_then(|a| a.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|art| art.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("Unknown Artist")
                        .to_string();

                    // Find associated release group or release
                    let mut album = "Unknown Album".to_string();
                    let mut release_id = None;
                    let mut track_count = None;

                    if let Some(releasegroups) =
                        rec.get("releasegroups").and_then(|rg| rg.as_array())
                    {
                        for rg in releasegroups {
                            if let Some(rg_title) = rg.get("title").and_then(|t| t.as_str()) {
                                if album == "Unknown Album" {
                                    album = rg_title.to_string();
                                }
                            }
                            if let Some(releases) =
                                rg.get("releases").and_then(|rel| rel.as_array())
                            {
                                if let Some(first_rel) = releases.first() {
                                    if let Some(rel_id) =
                                        first_rel.get("id").and_then(|id| id.as_str())
                                    {
                                        release_id = Some(rel_id.to_string());
                                    }
                                    if let Some(tc) =
                                        first_rel.get("track_count").and_then(|tc| tc.as_u64())
                                    {
                                        track_count = Some(tc as usize);
                                    }
                                    if let Some(rel_title) =
                                        first_rel.get("title").and_then(|t| t.as_str())
                                    {
                                        album = rel_title.to_string();
                                    }
                                }
                            }
                        }
                    }

                    return Ok(AcoustidMatch {
                        recording_id,
                        release_id,
                        title,
                        artist,
                        album,
                        score,
                        track_count,
                    });
                }
            }
        }

        Err("No recording metadata found for fingerprint in AcoustID database".to_string())
    }

    // ## Queries AcoustID web service with fingerprint and duration.
    pub fn lookup(
        fingerprint: &str,
        duration_secs: u32,
        client_key: Option<&str>,
    ) -> Result<AcoustidMatch, String> {
        let key = client_key.unwrap_or(DEFAULT_ACOUSTID_CLIENT_KEY);
        let url = format!(
            "https://api.acoustid.org/v2/lookup?client={}&duration={}&fingerprint={}&meta=recordings+releasegroups+releases+tracks+sources",
            key, duration_secs, fingerprint
        );

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build()
            .new_agent();

        let mut response = agent
            .get(&url)
            .header(
                "User-Agent",
                concat!(
                    "oxidisc/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/dral/OxiDisc)"
                ),
            )
            .call()
            .map_err(|err| format!("AcoustID lookup request failed: {err}"))?;

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|err| format!("Failed to read AcoustID response body: {err}"))?;

        Self::parse_acoustid_response(&body)
    }

    pub fn read_track_sample_and_fingerprint(
        device: &str,
        start_sector: u32,
        duration_secs: u32,
    ) -> Result<(String, u32), String> {
        if crate::mock_cd::is_mock_device(device) {
            let samples = crate::mock_cd::generate_mock_track_samples(1, duration_secs);
            let fp = Self::fingerprint_pcm16le(&samples, 44100, 2)?;
            return Ok((fp, duration_secs));
        }

        #[cfg(windows)]
        {
            crate::windows_cd::read_sample_pcm16le_windows(device, start_sector, duration_secs)
        }
        #[cfg(not(windows))]
        {
            let _ = (device, start_sector, duration_secs);
            Err("Acoustic drive reading is supported natively on Windows".to_string())
        }
    }

    // ## Performs acoustic recognition using Track 1 audio from the disc TOC.
    pub fn fetch_acoustid_metadata_with_toc(device: &str, toc: &CdToc) -> Result<CdInfo, String> {
        let track1_offset = toc
            .get_track_offset(1)
            .ok_or_else(|| "TOC has no Track 1".to_string())?;

        // 1. Read up to 45 seconds of Track 1 PCM audio
        let (fingerprint, duration_secs) =
            Self::read_track_sample_and_fingerprint(device, track1_offset, 45)?;

        // 2. Query AcoustID
        let acoustid_match = Self::lookup(&fingerprint, duration_secs, None)?;

        // 3. If MusicBrainz Release ID is available, query full release for complete tracklist and cover art
        if let Some(ref release_id) = acoustid_match.release_id {
            if let Ok(mut release_info) = Self::fetch_musicbrainz_release_info(release_id) {
                release_info.metadata_provider =
                    Some("AcoustID (Acoustic Fingerprint)".to_string());
                release_info.toc = Some(toc.clone());
                return Ok(release_info);
            }
        }

        // 4. Fallback to single track acoustid match info with TOC track count
        let track_count = toc.track_count();
        let mut tracks = Vec::with_capacity(track_count);
        tracks.push(acoustid_match.title);
        for i in 2..=track_count {
            tracks.push(format!("Track {}", i));
        }

        Ok(CdInfo {
            title: acoustid_match.album,
            artist: acoustid_match.artist,
            tracks,
            disc_id: acoustid_match
                .recording_id
                .unwrap_or_else(|| toc.cddb_disc_id_string()),
            album_cover_url: None,
            album_art_options: Vec::new(),
            metadata_error: None,
            metadata_provider: Some("AcoustID (Acoustic Fingerprint)".to_string()),
            toc: Some(toc.clone()),
        })
    }

    // ## Fetches full release information and artwork from MusicBrainz release ID.
    pub fn fetch_musicbrainz_release_info(release_id: &str) -> Result<CdInfo, String> {
        let url = format!(
            "https://musicbrainz.org/ws/2/release/{}?inc=recordings+artist-credits&fmt=json",
            release_id
        );

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .new_agent();

        let mut resp = agent
            .get(&url)
            .header(
                "User-Agent",
                concat!(
                    "oxidisc/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/dral/OxiDisc)"
                ),
            )
            .call()
            .map_err(|err| format!("MusicBrainz release lookup failed: {err}"))?;

        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|err| format!("Failed to read MusicBrainz release body: {err}"))?;

        let root: Value = serde_json::from_str(&body)
            .map_err(|err| format!("Failed to parse MusicBrainz JSON: {err}"))?;

        let album = root
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown Album")
            .to_string();

        let artist = root
            .get("artist-credit")
            .and_then(|ac| ac.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown Artist")
            .to_string();

        let mut tracks = Vec::new();
        if let Some(media) = root.get("media").and_then(|m| m.as_array()) {
            if let Some(first_medium) = media.first() {
                if let Some(t_arr) = first_medium.get("tracks").and_then(|t| t.as_array()) {
                    for (i, t) in t_arr.iter().enumerate() {
                        let title = t
                            .get("title")
                            .or_else(|| t.get("recording").and_then(|r| r.get("title")))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Track");
                        tracks.push(if title == "Track" {
                            format!("Track {}", i + 1)
                        } else {
                            title.to_string()
                        });
                    }
                }
            }
        }

        let cover_url = format!("https://coverartarchive.org/release/{}", release_id);
        let mut album_cover_url = None;
        let mut album_art_options = Vec::new();

        if let Ok(mut cover_resp) = agent
            .get(&cover_url)
            .header(
                "User-Agent",
                concat!(
                    "oxidisc/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/dral/OxiDisc)"
                ),
            )
            .call()
        {
            if let Ok(cover_body) = cover_resp.body_mut().read_to_string() {
                if let Ok(cover_json) = serde_json::from_str::<Value>(&cover_body) {
                    if let Some(images) = cover_json.get("images").and_then(|i| i.as_array()) {
                        if let Some(front) = images
                            .iter()
                            .find(|img| img.get("front").and_then(|f| f.as_bool()).unwrap_or(false))
                            .or_else(|| images.first())
                        {
                            album_cover_url = front
                                .get("image")
                                .and_then(|img| img.as_str())
                                .map(ToString::to_string);
                            if let Some(thumbnails) = front.get("thumbnails") {
                                if let Some(small) =
                                    thumbnails.get("small").and_then(|s| s.as_str())
                                {
                                    album_art_options.push(AlbumArtOption {
                                        key: "small".to_string(),
                                        label: "Small (250px)".to_string(),
                                        url: small.to_string(),
                                    });
                                }
                                if let Some(large) =
                                    thumbnails.get("large").and_then(|l| l.as_str())
                                {
                                    album_art_options.push(AlbumArtOption {
                                        key: "large".to_string(),
                                        label: "Large (500px)".to_string(),
                                        url: large.to_string(),
                                    });
                                }
                            }
                            if let Some(image) = front.get("image").and_then(|i| i.as_str()) {
                                album_art_options.push(AlbumArtOption {
                                    key: "original".to_string(),
                                    label: "Original".to_string(),
                                    url: image.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(CdInfo {
            title: album,
            artist,
            tracks,
            disc_id: release_id.to_string(),
            album_cover_url,
            album_art_options,
            metadata_error: None,
            metadata_provider: Some("AcoustID (Acoustic Fingerprint)".to_string()),
            toc: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_synthetic_sine_pcm16le() {
        // Generate 10 seconds of 440 Hz stereo sine wave
        let sample_rate = 44100;
        let channels = 2;
        let num_samples = sample_rate * 10;
        let mut samples = Vec::with_capacity(num_samples * channels);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample_val = ((2.0 * std::f32::consts::PI * 440.0 * t).sin() * 16000.0) as i16;
            samples.push(sample_val);
            samples.push(sample_val);
        }

        let fp = AcoustidClient::fingerprint_pcm16le(&samples, sample_rate as u32, channels as u32)
            .expect("Fingerprinting should succeed");
        assert!(!fp.is_empty());
        assert!(fp.starts_with("AQAA"));
    }

    #[test]
    fn test_parse_acoustid_response_success() {
        let json = r#"{
            "status": "ok",
            "results": [
                {
                    "id": "b3e9b1f2-4541-4770-8e1c-5d1b71cb6cf6",
                    "score": 0.98,
                    "recordings": [
                        {
                            "id": "78a0b0d3-3fb2-4bf1-8dfb-f6bc693685e1",
                            "title": "Money for Nothing",
                            "artists": [
                                {
                                    "id": "6ee360c9-2084-47ea-9ab6-fe36e409424e",
                                    "name": "Dire Straits"
                                }
                            ],
                            "releasegroups": [
                                {
                                    "id": "8b788ef9-804e-37bd-90a7-bc5b4f2c99de",
                                    "title": "Brothers in Arms",
                                    "type": "Album",
                                    "releases": [
                                        {
                                            "id": "97e68ad1-6677-4be7-8919-eb4d52187b5a",
                                            "title": "Brothers in Arms",
                                            "track_count": 9
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let res = AcoustidClient::parse_acoustid_response(json).expect("Should parse");
        assert_eq!(res.title, "Money for Nothing");
        assert_eq!(res.artist, "Dire Straits");
        assert_eq!(res.album, "Brothers in Arms");
        assert_eq!(
            res.release_id.as_deref(),
            Some("97e68ad1-6677-4be7-8919-eb4d52187b5a")
        );
        assert_eq!(res.track_count, Some(9));
        assert!(res.score > 0.95);
    }

    #[test]
    fn test_parse_acoustid_response_no_match() {
        let json = r#"{
            "status": "ok",
            "results": []
        }"#;

        let res = AcoustidClient::parse_acoustid_response(json);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_acoustid_response_error_status() {
        let json = r#"{
            "status": "error",
            "error": {
                "message": "Invalid API key"
            }
        }"#;

        let res = AcoustidClient::parse_acoustid_response(json);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Invalid API key"));
    }
}
