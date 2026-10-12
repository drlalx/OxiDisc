use crate::config::Config;
use crate::discid::CdToc;
use serde_json::Value;
use std::error::Error;
use std::time::Duration;

#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AlbumArtOption {
    pub key: String,
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct CdInfo {
    pub title: String,
    pub artist: String,
    pub tracks: Vec<String>,
    pub disc_id: String,
    pub album_cover_url: Option<String>,
    pub album_art_options: Vec<AlbumArtOption>,
    pub metadata_error: Option<String>,
    pub metadata_provider: Option<String>,
    pub toc: Option<CdToc>,
}

impl CdInfo {
    pub fn preferred_album_cover_url(&self, preference: &str) -> Option<&str> {
        let preferred_keys: &[&str] = match preference {
            "small" => &["small", "large", "original"],
            "large" => &["large", "original", "small"],
            "original" => &["original", "large", "small"],
            _ => &["large", "original", "small"],
        };

        for key in preferred_keys {
            if let Some(option) = self
                .album_art_options
                .iter()
                .find(|option| option.key == *key)
            {
                return Some(option.url.as_str());
            }
        }

        self.album_cover_url.as_deref()
    }
}

pub struct CdReader;

impl CdReader {
    pub fn active_device_path() -> String {
        Self::get_active_device_path()
    }

    pub fn get_active_device_path() -> String {
        // Highest priority: environment override
        if let Ok(dev) = std::env::var("CD_DEVICE") {
            let dev_trimmed = dev.trim();
            if !dev_trimmed.is_empty() {
                return dev_trimmed.to_string();
            }
        }

        // Next: configuration value
        let cfg = Config::load();
        let cfg_trimmed = cfg.device.trim();
        if !cfg_trimmed.is_empty() {
            #[cfg(unix)]
            {
                if Path::new(cfg_trimmed).exists() {
                    return cfg_trimmed.to_string();
                }
            }
            #[cfg(windows)]
            {
                return cfg_trimmed.to_string();
            }
        }

        // Fallback: platform-specific detection
        #[cfg(windows)]
        {
            crate::windows_cd::detect_windows_cd_drives()
                .first()
                .cloned()
                .unwrap_or_else(|| "D:".to_string())
        }

        #[cfg(unix)]
        {
            let candidates = ["/dev/cdrom", "/dev/sr0", "/dev/sr1"];
            for device in candidates {
                if Path::new(device).exists() {
                    return device.to_string();
                }
            }
            "/dev/sr0".to_string()
        }
    }

    pub fn detect_with_metadata_source(metadata_source: &str) -> Result<CdInfo, Box<dyn Error>> {
        Self::detect_impl(metadata_source)
    }

    pub fn read_disc_toc(device: &str) -> Result<CdToc, String> {
        if crate::mock_cd::is_mock_device(device) {
            return crate::mock_cd::get_mock_toc(device);
        }

        #[cfg(windows)]
        {
            crate::windows_cd::read_toc_windows(device)
        }

        #[cfg(unix)]
        {
            Self::read_toc_unix(device)
        }
    }

    fn detect_impl(metadata_source: &str) -> Result<CdInfo, Box<dyn Error>> {
        let device = Self::get_active_device_path();

        // 1. Try reading standard TOC
        let toc_result = Self::read_disc_toc(&device);

        let (toc, track_count) = match toc_result {
            Ok(toc) => {
                let count = toc.track_count();
                if count == 0 {
                    return Err("No audio tracks found on disc".into());
                }
                (Some(toc), count)
            }
            Err(err) => {
                #[cfg(unix)]
                {
                    if let Some(n) = Self::fallback_track_count(&device) {
                        (None, n)
                    } else {
                        return Err(format!("Failed to read TOC from {device}: {err}").into());
                    }
                }
                #[cfg(not(unix))]
                {
                    return Err(format!("Failed to read TOC from {device}: {err}").into());
                }
            }
        };

        if track_count == 0 {
            return Err("No audio tracks detected".into());
        }

        // Build baseline info
        let mut cd_info = Self::create_default_info_with_count("", track_count);
        cd_info.toc = toc.clone();

        // Metadata lookup
        match metadata_source {
            "musicbrainz" => {
                if let Some(ref toc_data) = toc {
                    match Self::fetch_musicbrainz_metadata_with_toc(toc_data) {
                        Ok(mut info) => {
                            info.metadata_provider = Some("MusicBrainz".to_string());
                            info.toc = Some(toc_data.clone());
                            cd_info = info;
                        }
                        Err(err) => cd_info.metadata_error = Some(err),
                    }
                }
            }
            "gnudb" => {
                if let Some(ref toc_data) = toc {
                    match Self::fetch_gnudb_metadata_with_toc(toc_data) {
                        Ok(mut info) => {
                            info.metadata_provider = Some("GnuDB (FreeDB)".to_string());
                            info.toc = Some(toc_data.clone());
                            cd_info = info;
                        }
                        Err(err) => cd_info.metadata_error = Some(err),
                    }
                }
            }
            "acoustid" => {
                if let Some(ref toc_data) = toc {
                    match crate::acoustid::AcoustidClient::fetch_acoustid_metadata_with_toc(&device, toc_data) {
                        Ok(mut info) => {
                            info.metadata_provider = Some("AcoustID (Acoustic Fingerprint)".to_string());
                            info.toc = Some(toc_data.clone());
                            cd_info = info;
                        }
                        Err(err) => cd_info.metadata_error = Some(err),
                    }
                }
            }
            _ /* "auto" */ => {
                if let Some(ref toc_data) = toc {
                    match Self::fetch_auto_metadata_with_toc(&device, toc_data) {
                        Ok(mut info) => {
                            info.toc = Some(toc_data.clone());
                            cd_info = info;
                        }
                        Err(err) => cd_info.metadata_error = Some(err),
                    }
                }
            }
        }

        Ok(cd_info)
    }

    #[cfg(unix)]
    fn read_toc_unix(device: &str) -> Result<CdToc, String> {
        // ioctl constants from linux/cdrom.h
        const CDROMREADTOCHDR: libc::Ioctl = 0x5305;
        const CDROMREADTOCENTRY: libc::Ioctl = 0x5306;
        const CDROM_LBA: u8 = 0x01;

        #[repr(C)]
        struct CdromTocHdr {
            cdth_trk0: libc::c_uchar,
            cdth_trk1: libc::c_uchar,
        }

        #[repr(C)]
        struct CdromTocEntry {
            cdte_track: libc::c_uchar,
            cdte_adr_ctrl: libc::c_uchar,
            cdte_format: libc::c_uchar,
            cdte_addr: [libc::c_uchar; 4], // LBA or MSF union
            cdte_datamode: libc::c_uchar,
        }

        let f = File::open(device).map_err(|e| format!("Cannot open device {device}: {e}"))?;
        let fd = f.as_raw_fd();
        let mut hdr = CdromTocHdr {
            cdth_trk0: 0,
            cdth_trk1: 0,
        };

        let ret =
            unsafe { libc::ioctl(fd, CDROMREADTOCHDR, &mut hdr as *mut _ as *mut libc::c_void) };
        if ret != 0 {
            return Err(format!(
                "ioctl CDROMREADTOCHDR failed: {}",
                io::Error::last_os_error()
            ));
        }

        let first = hdr.cdth_trk0;
        let last = hdr.cdth_trk1;
        if first == 0 || last < first || last > 99 {
            return Err(format!("Invalid track range: {first}..={last}"));
        }

        let mut toc = CdToc::new(first, last, 0)?;

        for t in first..=last {
            let mut entry: CdromTocEntry = unsafe { std::mem::zeroed() };
            entry.cdte_track = t;
            entry.cdte_format = CDROM_LBA;

            let ret = unsafe {
                libc::ioctl(
                    fd,
                    CDROMREADTOCENTRY,
                    &mut entry as *mut _ as *mut libc::c_void,
                )
            };
            if ret == 0 {
                let lba = i32::from_ne_bytes(entry.cdte_addr);
                let sector = (lba.max(0) as u32) + 150;
                let _ = toc.set_track_offset(t, sector);
            }
        }

        // Lead-out track is 0xAA (170)
        let mut leadout_entry: CdromTocEntry = unsafe { std::mem::zeroed() };
        leadout_entry.cdte_track = 0xAA;
        leadout_entry.cdte_format = CDROM_LBA;
        let ret = unsafe {
            libc::ioctl(
                fd,
                CDROMREADTOCENTRY,
                &mut leadout_entry as *mut _ as *mut libc::c_void,
            )
        };
        if ret == 0 {
            let lba = i32::from_ne_bytes(leadout_entry.cdte_addr);
            toc.leadout_offset = (lba.max(0) as u32) + 150;
        }

        Ok(toc)
    }

    #[cfg(unix)]
    fn fallback_track_count(device: &str) -> Option<usize> {
        let mut cmd = Command::new("cdparanoia");
        cmd.arg("-Q").arg("-d").arg(device);
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(count) = Self::parse_cdparanoia_q_for_track_count(&text) {
                    return Some(count);
                }
            }
        }

        // As a last resort, cd-discid
        if let Ok(o) = Command::new("cd-discid").arg(device).output() {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout);
                let mut it = s.split_whitespace();
                let _ = it.next();
                if let Some(tok) = it.next() {
                    if let Ok(n) = tok.parse::<usize>() {
                        if n > 0 {
                            return Some(n);
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(unix)]
    fn parse_cdparanoia_q_for_track_count(output: &str) -> Option<usize> {
        let mut count = 0usize;
        for line in output.lines() {
            let s = line.trim_start();
            let mut chars = s.chars();
            match chars.next() {
                Some(c) if c.is_ascii_digit() => {
                    if s.contains('.') {
                        count += 1;
                    }
                }
                _ => {}
            }
        }
        if count > 0 { Some(count) } else { None }
    }

    pub fn fetch_musicbrainz_metadata_with_toc(toc: &CdToc) -> Result<CdInfo, String> {
        let mbid = toc.calculate_disc_id();
        let toc_param = toc.toc_string().replace(' ', "+");

        let url = format!(
            "https://musicbrainz.org/ws/2/discid/{}?toc={}&inc=artists+recordings+release-groups&fmt=json",
            mbid, toc_param
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
            .map_err(|err| format!("MusicBrainz request failed: {err}"))?;

        let json: Value = resp
            .body_mut()
            .read_json::<Value>()
            .map_err(|err| format!("MusicBrainz response was not valid JSON: {err}"))?;

        let releases = json
            .get("releases")
            .and_then(|releases| releases.as_array())
            .ok_or_else(|| "MusicBrainz response did not include releases".to_string())?;

        let first = releases
            .first()
            .ok_or_else(|| format!("MusicBrainz found no releases for disc ID {mbid}"))?;

        // Fetch cover art from Cover Art Archive
        let mut album_cover_url = None;
        let mut album_art_options = Vec::new();
        if let Some(release_mbid) = first.get("id").and_then(|id| id.as_str()) {
            let cover_art_url = format!("https://coverartarchive.org/release/{}", release_mbid);
            if let Ok(mut cover_resp) = agent.get(&cover_art_url).call() {
                if let Ok(cover_json) = cover_resp.body_mut().read_json::<Value>() {
                    if let Some(images) = cover_json.get("images").and_then(|i| i.as_array()) {
                        let front_image = images.iter().find(|img| {
                            img.get("front").and_then(|v| v.as_bool()).unwrap_or(false)
                        });
                        if let Some(img) = front_image {
                            Self::push_album_art_option(
                                &mut album_art_options,
                                "small",
                                "Small thumbnail",
                                img.get("thumbnails").and_then(|t| t.get("small")),
                            );
                            Self::push_album_art_option(
                                &mut album_art_options,
                                "large",
                                "Large thumbnail",
                                img.get("thumbnails").and_then(|t| t.get("large")),
                            );
                            Self::push_album_art_option(
                                &mut album_art_options,
                                "original",
                                "Original image",
                                img.get("image"),
                            );
                        }

                        let preferred_size = Config::load().album_art_size_preference;
                        album_cover_url =
                            Self::preferred_album_art_url(&album_art_options, &preferred_size)
                                .map(ToOwned::to_owned);
                    }
                }
            }
        }

        let album = first
            .get("title")
            .and_then(|title| title.as_str())
            .ok_or_else(|| "MusicBrainz release did not include an album title".to_string())?
            .to_string();

        let artist = first
            .get("artist-credit")
            .and_then(|ac| ac.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("name").and_then(|n| n.as_str()))
            .unwrap_or("Unknown Artist")
            .to_string();

        let media = first
            .get("media")
            .and_then(|m| m.as_array())
            .and_then(|arr| arr.first());

        let tracks_v = media
            .and_then(|m| m.get("tracks"))
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks = Vec::new();
        for (i, t) in tracks_v.iter().enumerate() {
            let title_str = t
                .get("title")
                .or_else(|| t.get("recording").and_then(|r| r.get("title")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Track {}", i + 1));
            tracks.push(title_str);
        }

        if tracks.is_empty() {
            let count = toc.track_count();
            tracks = (1..=count).map(|i| format!("Track {}", i)).collect();
        }

        Ok(CdInfo {
            title: album,
            artist,
            tracks,
            disc_id: mbid,
            album_cover_url,
            album_art_options,
            metadata_error: None,
            metadata_provider: Some("MusicBrainz".to_string()),
            toc: Some(toc.clone()),
        })
    }

    pub fn fetch_auto_metadata_with_toc(device: &str, toc: &CdToc) -> Result<CdInfo, String> {
        // 1. First attempt: MusicBrainz (includes rich metadata & Cover Art Archive images)
        match Self::fetch_musicbrainz_metadata_with_toc(toc) {
            Ok(mut info) => {
                info.metadata_provider = Some("MusicBrainz".to_string());
                Ok(info)
            }
            Err(mb_err) => {
                // 2. Second attempt: GnuDB (FreeDB community database)
                match Self::fetch_gnudb_metadata_with_toc(toc) {
                    Ok(mut gnudb_info) => {
                        gnudb_info.metadata_provider = Some("GnuDB (FreeDB)".to_string());
                        Ok(gnudb_info)
                    }
                    Err(gnudb_err) => {
                        // 3. Third attempt: AcoustID (Acoustic audio fingerprinting from Track 1)
                        match crate::acoustid::AcoustidClient::fetch_acoustid_metadata_with_toc(
                            device, toc,
                        ) {
                            Ok(mut acoustid_info) => {
                                acoustid_info.metadata_provider =
                                    Some("AcoustID (Acoustic Fingerprint)".to_string());
                                Ok(acoustid_info)
                            }
                            Err(acoustid_err) => {
                                if crate::mock_cd::is_mock_device(device) {
                                    Ok(crate::mock_cd::get_mock_fallback_info(toc))
                                } else {
                                    Err(format!(
                                        "Auto-lookup failed (MusicBrainz: {mb_err}; GnuDB: {gnudb_err}; AcoustID: {acoustid_err})"
                                    ))
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn fetch_gnudb_metadata_with_toc(toc: &CdToc) -> Result<CdInfo, String> {
        let disc_id_hex = toc.cddb_disc_id_string();
        let query_params = toc.cddb_query_params();

        let query_url = format!(
            "http://gnudb.gnudb.org/~cddb/cddb.cgi?cmd=cddb+query+{}&hello=anonymous+localhost+oxidisc+2.0.0&proto=6",
            query_params
        );

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .new_agent();

        let mut query_resp = agent
            .get(&query_url)
            .header(
                "User-Agent",
                concat!(
                    "oxidisc/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/dral/OxiDisc)"
                ),
            )
            .call()
            .map_err(|err| format!("GnuDB query request failed: {err}"))?;

        let query_body = query_resp
            .body_mut()
            .read_to_string()
            .map_err(|err| format!("Failed to read GnuDB query response: {err}"))?;

        let cddb_match = Self::parse_cddb_query_response(&query_body)?;

        let read_url = format!(
            "http://gnudb.gnudb.org/~cddb/cddb.cgi?cmd=cddb+read+{}+{}&hello=anonymous+localhost+oxidisc+2.0.0&proto=6",
            cddb_match.category, cddb_match.disc_id
        );

        let mut read_resp = agent
            .get(&read_url)
            .header(
                "User-Agent",
                concat!(
                    "oxidisc/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/dral/OxiDisc)"
                ),
            )
            .call()
            .map_err(|err| format!("GnuDB read request failed: {err}"))?;

        let read_body = read_resp
            .body_mut()
            .read_to_string()
            .map_err(|err| format!("Failed to read GnuDB entry response: {err}"))?;

        let mut cd_info =
            Self::parse_cddb_read_response(&read_body, &disc_id_hex, toc.track_count())?;
        cd_info.toc = Some(toc.clone());
        Ok(cd_info)
    }

    pub fn parse_cddb_query_response(response: &str) -> Result<CddbMatch, String> {
        let first_line = response.lines().next().unwrap_or("").trim();
        let mut tokens = first_line.split_whitespace();
        let code = tokens.next().unwrap_or("");

        match code {
            "200" => {
                let category = tokens
                    .next()
                    .ok_or_else(|| "GnuDB response missing category".to_string())?
                    .to_string();
                let disc_id = tokens
                    .next()
                    .ok_or_else(|| "GnuDB response missing disc ID".to_string())?
                    .to_string();
                let title = tokens.collect::<Vec<&str>>().join(" ");
                Ok(CddbMatch {
                    category,
                    disc_id,
                    title,
                })
            }
            "210" | "211" => {
                for line in response.lines().skip(1) {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed == "." {
                        continue;
                    }
                    let mut match_tokens = trimmed.split_whitespace();
                    if let (Some(cat), Some(id)) = (match_tokens.next(), match_tokens.next()) {
                        let title = match_tokens.collect::<Vec<&str>>().join(" ");
                        return Ok(CddbMatch {
                            category: cat.to_string(),
                            disc_id: id.to_string(),
                            title,
                        });
                    }
                }
                Err("GnuDB returned matches list but no valid entry found".to_string())
            }
            "202" => Err("No match found in GnuDB".to_string()),
            _ => Err(format!("GnuDB query error: {}", first_line)),
        }
    }

    pub fn parse_cddb_read_response(
        response: &str,
        disc_id: &str,
        track_count: usize,
    ) -> Result<CdInfo, String> {
        let first_line = response.lines().next().unwrap_or("").trim();
        if !first_line.starts_with("210") {
            return Err(format!("GnuDB read failed: {first_line}"));
        }

        let mut dtitle = String::new();
        let mut track_map: std::collections::BTreeMap<usize, String> =
            std::collections::BTreeMap::new();

        for line in response.lines().skip(1) {
            let line = line.trim();
            if line == "." {
                break;
            }
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();

                if key == "DTITLE" {
                    if !dtitle.is_empty() {
                        dtitle.push(' ');
                    }
                    dtitle.push_str(val);
                } else if let Some(idx_str) = key.strip_prefix("TTITLE") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        let entry = track_map.entry(idx).or_default();
                        entry.push_str(val);
                    }
                }
            }
        }

        let (artist, title) = if let Some((art, tit)) = dtitle.split_once(" / ") {
            (art.trim().to_string(), tit.trim().to_string())
        } else if let Some((art, tit)) = dtitle.split_once('/') {
            (art.trim().to_string(), tit.trim().to_string())
        } else if !dtitle.is_empty() {
            ("Unknown Artist".to_string(), dtitle.trim().to_string())
        } else {
            ("Unknown Artist".to_string(), "Unknown Album".to_string())
        };

        let mut tracks = Vec::with_capacity(track_count);
        for i in 0..track_count {
            if let Some(track_title) = track_map.get(&i) {
                if !track_title.trim().is_empty() {
                    tracks.push(track_title.trim().to_string());
                    continue;
                }
            }
            tracks.push(format!("Track {}", i + 1));
        }

        Ok(CdInfo {
            title,
            artist,
            tracks,
            disc_id: disc_id.to_string(),
            album_cover_url: None,
            album_art_options: Vec::new(),
            metadata_error: None,
            metadata_provider: Some("GnuDB (FreeDB)".to_string()),
            toc: None,
        })
    }

    fn create_default_info_with_count(disc_id: &str, track_count: usize) -> CdInfo {
        let tracks: Vec<String> = (1..=track_count).map(|i| format!("Track {}", i)).collect();

        CdInfo {
            title: "Unknown Album".to_string(),
            artist: "Unknown Artist".to_string(),
            tracks,
            disc_id: disc_id.to_string(),
            album_cover_url: None,
            album_art_options: Vec::new(),
            metadata_error: None,
            metadata_provider: None,
            toc: None,
        }
    }

    fn push_album_art_option(
        options: &mut Vec<AlbumArtOption>,
        key: &str,
        label: &str,
        value: Option<&Value>,
    ) {
        let Some(url) = value.and_then(|value| value.as_str()) else {
            return;
        };

        if options.iter().any(|option| option.url == url) {
            return;
        }

        options.push(AlbumArtOption {
            key: key.to_string(),
            label: label.to_string(),
            url: url.to_string(),
        });
    }

    fn preferred_album_art_url<'a>(
        options: &'a [AlbumArtOption],
        preference: &str,
    ) -> Option<&'a str> {
        let preferred_keys: &[&str] = match preference {
            "small" => &["small", "large", "original"],
            "large" => &["large", "original", "small"],
            "original" => &["original", "large", "small"],
            _ => &["large", "original", "small"],
        };

        for key in preferred_keys {
            if let Some(option) = options.iter().find(|option| option.key == *key) {
                return Some(option.url.as_str());
            }
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CddbMatch {
    pub category: String,
    pub disc_id: String,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_info_with_count() {
        let info = CdReader::create_default_info_with_count("test_id", 4);
        assert_eq!(info.tracks.len(), 4);
        assert_eq!(info.tracks[0], "Track 1");
        assert_eq!(info.tracks[3], "Track 4");
        assert_eq!(info.disc_id, "test_id");
        assert_eq!(info.title, "Unknown Album");
        assert_eq!(info.artist, "Unknown Artist");
    }

    #[test]
    fn test_preferred_album_cover_url() {
        let options = vec![
            AlbumArtOption {
                key: "small".to_string(),
                label: "Small".to_string(),
                url: "http://example.com/small.jpg".to_string(),
            },
            AlbumArtOption {
                key: "large".to_string(),
                label: "Large".to_string(),
                url: "http://example.com/large.jpg".to_string(),
            },
        ];

        assert_eq!(
            CdReader::preferred_album_art_url(&options, "small"),
            Some("http://example.com/small.jpg")
        );
        assert_eq!(
            CdReader::preferred_album_art_url(&options, "large"),
            Some("http://example.com/large.jpg")
        );
        assert_eq!(
            CdReader::preferred_album_art_url(&options, "nonexistent"),
            Some("http://example.com/large.jpg")
        );
    }

    #[test]
    fn test_parse_cddb_query_response_200_exact() {
        let response = "200 rock a10c2e0c Pink Floyd / The Wall\n";
        let cddb_match = CdReader::parse_cddb_query_response(response).unwrap();
        assert_eq!(cddb_match.category, "rock");
        assert_eq!(cddb_match.disc_id, "a10c2e0c");
        assert_eq!(cddb_match.title, "Pink Floyd / The Wall");
    }

    #[test]
    fn test_parse_cddb_query_response_210_multiple() {
        let response = "210 Exact matches found\nrock a10c2e0c Pink Floyd / The Wall (CD 1)\nrock b20d3f1d Pink Floyd / The Wall (Remastered)\n.\n";
        let cddb_match = CdReader::parse_cddb_query_response(response).unwrap();
        assert_eq!(cddb_match.category, "rock");
        assert_eq!(cddb_match.disc_id, "a10c2e0c");
        assert_eq!(cddb_match.title, "Pink Floyd / The Wall (CD 1)");
    }

    #[test]
    fn test_parse_cddb_query_response_202_no_match() {
        let response = "202 No match found\n";
        assert!(CdReader::parse_cddb_query_response(response).is_err());
    }

    #[test]
    fn test_parse_cddb_read_response() {
        let response = "210 rock a10c2e0c CD database entry follows (until .)\n# Revision: 1\n# Submitted via: oxidisc\nDTITLE=Dire Straits / Brothers in Arms\nDYEAR=1985\nDGENRE=Rock\nTTITLE0=So Far Away\nTTITLE1=Money for Nothing\nTTITLE2=Walk of Life\n.\n";
        let cd_info = CdReader::parse_cddb_read_response(response, "a10c2e0c", 3).unwrap();
        assert_eq!(cd_info.artist, "Dire Straits");
        assert_eq!(cd_info.title, "Brothers in Arms");
        assert_eq!(cd_info.metadata_provider.as_deref(), Some("GnuDB (FreeDB)"));
        assert_eq!(cd_info.tracks.len(), 3);
        assert_eq!(cd_info.tracks[0], "So Far Away");
        assert_eq!(cd_info.tracks[1], "Money for Nothing");
        assert_eq!(cd_info.tracks[2], "Walk of Life");
    }
}
