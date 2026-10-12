use crate::cd_reader::CdInfo;
use crate::config::Config;
use crate::wav::{create_wav_header, track_pcm_bytes, track_sector_range};
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
#[cfg(windows)]
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(unix)]
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum RipMessage {
    Progress(f64, String),
    TrackComplete(usize),
    Success,
    Error(String),
}

pub struct Ripper {
    config: Config,
    output_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
    current_child: Arc<Mutex<Option<Child>>>,
    sender: UnboundedSender<RipMessage>,
    runtime: Arc<Runtime>,
}

fn sanitize_path_component(value: &str, fallback: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut pending_space = false;

    for ch in value.chars() {
        let normalized = match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            _ if ch.is_control() => ' ',
            _ if ch.is_whitespace() => ' ',
            _ => ch,
        };

        if normalized == ' ' {
            pending_space = !sanitized.is_empty();
            continue;
        }

        if pending_space {
            sanitized.push(' ');
            pending_space = false;
        }

        sanitized.push(normalized);
    }

    let sanitized = sanitized
        .trim_matches(|c: char| c == ' ' || c == '.')
        .to_string();

    if sanitized.is_empty()
        || sanitized == "."
        || sanitized == ".."
        || is_windows_reserved_name(&sanitized)
    {
        fallback.to_string()
    } else {
        sanitized
    }
}

fn sanitize_track_name(track_name: &str, track_num: usize) -> String {
    sanitize_path_component(track_name, &format!("Track {:02}", track_num))
}

fn final_output_path(encoder: &str, track_name: &str, output_dir: &Path) -> PathBuf {
    let extension = match encoder {
        "flac" => "flac",
        "mp3" => "mp3",
        "ogg" => "ogg",
        "opus" => "opus",
        _ => "wav",
    };

    output_dir.join(format!("{}.{}", track_name, extension))
}

fn working_wav_path(
    encoder: &str,
    track_num: usize,
    track_name: &str,
    output_dir: &Path,
) -> PathBuf {
    if encoder == "wav" {
        final_output_path(encoder, track_name, output_dir)
    } else {
        output_dir.join(format!("track{:02}.wav", track_num))
    }
}

fn is_windows_reserved_name(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();

    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CLOCK$"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

impl Ripper {
    pub fn new(
        config: Config,
        output_dir: PathBuf,
        sender: UnboundedSender<RipMessage>,
    ) -> Result<Self, Box<dyn Error>> {
        let runtime = Arc::new(Runtime::new()?);
        Ok(Self {
            config,
            output_dir,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            current_child: Arc::new(Mutex::new(None)),
            sender,
            runtime,
        })
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.current_child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
            guard.take();
        }
    }

    pub fn rip(&self, cd_info: &CdInfo) {
        let album_name = sanitize_path_component(&cd_info.title, "Unknown Album");
        let album_dir = self.output_dir.join(album_name);
        if let Err(e) = std::fs::create_dir_all(&album_dir) {
            let _ = self.sender.send(RipMessage::Error(format!(
                "Failed to create directory: {}",
                e
            )));
            return;
        }

        if cd_info.tracks.is_empty() {
            let _ = self.sender.send(RipMessage::Error(
                "No tracks found to rip. Please detect again.".into(),
            ));
            return;
        }

        self.maybe_save_album_art(cd_info, &album_dir);

        let total_tracks = cd_info.tracks.len();
        for (i, track_name) in cd_info.tracks.iter().enumerate() {
            let track_num = i + 1;
            if self.cancel_flag.load(Ordering::SeqCst) {
                let _ = self
                    .sender
                    .send(RipMessage::Error("Ripping cancelled".into()));
                return;
            }

            let progress = (track_num as f64 - 1.0) / (total_tracks as f64);
            let message = format!("Ripping track {} of {}...", track_num, total_tracks);
            let _ = self.sender.send(RipMessage::Progress(progress, message));

            let track_name_clone = track_name.clone();
            let album_dir_clone = album_dir.clone();
            let runtime = self.runtime.clone();
            let cd_info_clone = cd_info.clone();

            let res = runtime.block_on(self.rip_track(
                track_num,
                total_tracks,
                &track_name_clone,
                &album_dir_clone,
                &cd_info_clone,
            ));

            if let Err(e) = res {
                let _ = self.sender.send(RipMessage::Error(format!(
                    "Failed to rip track {}: {}",
                    track_num, e
                )));
                return;
            }
            let _ = self.sender.send(RipMessage::TrackComplete(track_num));
        }

        let _ = self.sender.send(RipMessage::Success);
    }

    fn maybe_save_album_art(&self, cd_info: &CdInfo, album_dir: &Path) {
        if self.config.album_art_download_behavior != "save-with-rip" {
            return;
        }

        let Some(url) = &cd_info.album_cover_url else {
            return;
        };

        if let Err(err) = Self::download_album_art(url, album_dir) {
            eprintln!("Failed to save album art: {}", err);
        }
    }

    fn download_album_art(url: &str, album_dir: &Path) -> Result<(), Box<dyn Error>> {
        let mut response = ureq::get(url).call()?;
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_ascii_lowercase();

        let bytes = response
            .body_mut()
            .with_config()
            .limit(50 * 1024 * 1024)
            .read_to_vec()?;

        let extension = if content_type.contains("png") {
            "png"
        } else if content_type.contains("webp") {
            "webp"
        } else {
            "jpg"
        };

        std::fs::write(album_dir.join(format!("cover.{}", extension)), bytes)?;
        Ok(())
    }

    async fn rip_track(
        &self,
        track_num: usize,
        total_tracks: usize,
        track_name: &str,
        output_dir: &PathBuf,
        cd_info: &CdInfo,
    ) -> Result<(), Box<dyn Error>> {
        let safe_track_name = sanitize_track_name(track_name, track_num);
        let final_output = final_output_path(&self.config.encoder, &safe_track_name, output_dir);
        let wav_file = working_wav_path(
            &self.config.encoder,
            track_num,
            &safe_track_name,
            output_dir,
        );

        // Windows native extraction or Unix GStreamer/cdparanoia
        #[cfg(windows)]
        {
            self.rip_track_windows(track_num, total_tracks, &wav_file, cd_info)?;
        }

        #[cfg(unix)]
        {
            self.rip_track_unix(track_num, total_tracks, &wav_file, cd_info)?;
        }

        // Encode based on selected format
        let output_file = match self.config.encoder.as_str() {
            "flac" => self.encode_flac(&wav_file, &safe_track_name, output_dir)?,
            "mp3" => self.encode_mp3(&wav_file, &safe_track_name, output_dir)?,
            "ogg" => self.encode_ogg(&wav_file, &safe_track_name, output_dir)?,
            "opus" => self.encode_opus(&wav_file, &safe_track_name, output_dir)?,
            "wav" => final_output,
            _ => wav_file.clone(),
        };

        // Remove temporary WAV if we encoded to another format
        if output_file != wav_file && wav_file.exists() {
            let _ = std::fs::remove_file(&wav_file);
        }

        Ok(())
    }

    #[cfg(windows)]
    fn rip_track_windows(
        &self,
        track_num: usize,
        total_tracks: usize,
        wav_file: &PathBuf,
        cd_info: &CdInfo,
    ) -> Result<(), Box<dyn Error>> {
        let toc = match &cd_info.toc {
            Some(toc) => toc.clone(),
            None => {
                let dev = crate::cd_reader::CdReader::active_device_path();
                crate::windows_cd::read_toc_windows(&dev)?
            }
        };

        let (start_sector, end_sector) = track_sector_range(&toc, track_num)?;
        let expected_pcm_bytes = track_pcm_bytes(start_sector, end_sector);

        let mut file = File::create(wav_file)?;
        let header = create_wav_header(expected_pcm_bytes as u32);
        file.write_all(&header)?;

        let dev = crate::cd_reader::CdReader::active_device_path();
        let sender = self.sender.clone();
        let track_base_fraction = (track_num as f64 - 1.0) / (total_tracks as f64);
        let track_slice_fraction = 1.0 / (total_tracks as f64);

        crate::windows_cd::rip_sectors_windows(
            &dev,
            start_sector,
            end_sector,
            &self.cancel_flag,
            |track_frac| {
                let overall = track_base_fraction + (track_frac * track_slice_fraction);
                let msg = format!(
                    "Ripping track {} of {} ({:.0}%)...",
                    track_num,
                    total_tracks,
                    track_frac * 100.0
                );
                let _ = sender.send(RipMessage::Progress(overall, msg));
            },
            &mut file,
        )?;

        // Ensure file is flushed and sync header if written bytes differed slightly
        file.flush()?;
        let actual_len = file.metadata()?.len();
        if actual_len >= 44 {
            let actual_pcm_len = (actual_len - 44) as u32;
            if actual_pcm_len != (expected_pcm_bytes as u32) {
                let updated_header = create_wav_header(actual_pcm_len);
                file.seek(SeekFrom::Start(0))?;
                file.write_all(&updated_header)?;
                file.flush()?;
            }
        }

        Ok(())
    }

    #[cfg(unix)]
    fn rip_track_unix(
        &self,
        track_num: usize,
        total_tracks: usize,
        wav_file: &PathBuf,
        cd_info: &CdInfo,
    ) -> Result<(), Box<dyn Error>> {
        let dev = crate::cd_reader::CdReader::active_device_path();
        if crate::mock_cd::is_mock_device(&dev)
            || crate::mock_cd::is_mock_device(&self.config.device)
        {
            let toc = match &cd_info.toc {
                Some(toc) => toc.clone(),
                None => crate::mock_cd::get_mock_toc(&dev)?,
            };

            let (start_sector, end_sector) = track_sector_range(&toc, track_num)?;
            let expected_pcm_bytes = track_pcm_bytes(start_sector, end_sector);

            let mut file = File::create(wav_file)?;
            let header = create_wav_header(expected_pcm_bytes as u32);
            file.write_all(&header)?;

            let sender = self.sender.clone();
            let track_base_fraction = (track_num as f64 - 1.0) / (total_tracks as f64);
            let track_slice_fraction = 1.0 / (total_tracks as f64);

            crate::mock_cd::rip_sectors_mock(
                start_sector,
                end_sector,
                &self.cancel_flag,
                |track_frac| {
                    let overall = track_base_fraction + (track_frac * track_slice_fraction);
                    let msg = format!(
                        "Ripping track {} of {} ({:.0}%)...",
                        track_num,
                        total_tracks,
                        track_frac * 100.0
                    );
                    let _ = sender.send(RipMessage::Progress(overall, msg));
                },
                &mut file,
            )?;

            file.flush()?;
            return Ok(());
        }

        // Rip track with cdparanoia CLI
        let child = Command::new("cdparanoia")
            .arg("-d")
            .arg(&self.config.device)
            .arg(format!("{}", track_num))
            .arg(wav_file)
            .spawn()?;

        if let Ok(mut guard) = self.current_child.lock() {
            *guard = Some(child);
        }

        loop {
            if self.cancel_flag.load(Ordering::SeqCst) {
                self.cancel();
                return Err("Ripping cancelled".into());
            }

            let completed = if let Ok(mut guard) = self.current_child.lock() {
                if let Some(c) = guard.as_mut() {
                    if let Ok(Some(status)) = c.try_wait() {
                        let ok = status.success();
                        guard.take();
                        Some(ok)
                    } else {
                        None
                    }
                } else {
                    Some(false)
                }
            } else {
                Some(false)
            };

            if let Some(ok) = completed {
                if ok {
                    return Ok(());
                } else {
                    return Err(format!("Failed to rip track {} with cdparanoia", track_num).into());
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn encode_flac(
        &self,
        input: &PathBuf,
        track_name: &str,
        output_dir: &PathBuf,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let output = output_dir.join(format!("{}.flac", track_name));

        // 1. Try Pure-Rust native FLAC encoding (0 external dependencies)
        if let Ok(()) = Self::encode_flac_pure_rust(input, &output) {
            return Ok(output);
        }

        // 2. Fallback to CLI 'flac'
        if is_command_available("flac") {
            let res = Command::new("flac")
                .arg("-8")
                .arg("-f")
                .arg(input)
                .arg("-o")
                .arg(&output)
                .status();
            if let Ok(status) = res {
                if status.success() {
                    return Ok(output);
                }
            }
        }

        // 3. Fallback to 'ffmpeg'
        if is_command_available("ffmpeg") {
            let res = Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(input)
                .arg("-c:a")
                .arg("flac")
                .arg(&output)
                .status();
            if let Ok(status) = res {
                if status.success() {
                    return Ok(output);
                }
            }
        }

        Err("FLAC encoding failed. Please choose WAV format or install 'flac'/'ffmpeg'.".into())
    }

    fn encode_flac_pure_rust(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
        use flacenc::error::Verify;

        let mut file = File::open(input)?;
        let mut header = [0u8; 44];
        file.read_exact(&mut header)?;

        let mut pcm_bytes = Vec::new();
        file.read_to_end(&mut pcm_bytes)?;

        let channels = 2usize;
        let bits_per_sample = 16usize;
        let sample_rate = 44100usize;

        let mut samples = Vec::with_capacity(pcm_bytes.len() / 2);
        for chunk in pcm_bytes.chunks_exact(2) {
            samples.push(i16::from_le_bytes([chunk[0], chunk[1]]) as i32);
        }

        let source = flacenc::source::MemSource::from_samples(
            &samples,
            channels,
            bits_per_sample,
            sample_rate,
        );
        let config = flacenc::config::Encoder::default()
            .into_verified()
            .map_err(|e| format!("flacenc config error: {:?}", e))?;
        let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, 4096)
            .map_err(|e| format!("flacenc error: {:?}", e))?;

        let mut sink = flacenc::bitsink::ByteSink::new();
        flacenc::component::BitRepr::write(&flac_stream, &mut sink)
            .map_err(|e| format!("flacenc bit sink error: {:?}", e))?;

        let mut out_file = File::create(output)?;
        out_file.write_all(sink.as_slice())?;
        out_file.flush()?;

        Ok(())
    }

    fn encode_mp3(
        &self,
        input: &PathBuf,
        track_name: &str,
        output_dir: &PathBuf,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let output = output_dir.join(format!("{}.mp3", track_name));
        let bitrate = if self.config.bitrate.trim().is_empty() {
            "320"
        } else {
            &self.config.bitrate
        };

        // 1. Try 'lame' CLI
        if is_command_available("lame") {
            let res = Command::new("lame")
                .arg("-b")
                .arg(bitrate)
                .arg(input)
                .arg(&output)
                .status();

            match res {
                Ok(status) if status.success() => return Ok(output),
                Ok(status) => {
                    eprintln!("LAME returned exit code {:?}", status.code());
                }
                Err(e) => {
                    eprintln!("LAME execution failed: {e}");
                }
            }
        }

        // 2. Fallback to 'ffmpeg'
        if is_command_available("ffmpeg") {
            let res = Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(input)
                .arg("-c:a")
                .arg("libmp3lame")
                .arg("-b:a")
                .arg(format!("{}k", bitrate))
                .arg(&output)
                .status();

            if let Ok(status) = res {
                if status.success() {
                    return Ok(output);
                }
            }
        }

        Err(
            "MP3-Encoder ('lame' oder 'ffmpeg') nicht im PATH gefunden. Bitte LAME oder FFmpeg installieren (z. B. 'winget install Gyan.FFmpeg') oder FLAC/WAV wählen."
                .into(),
        )
    }

    fn encode_ogg(
        &self,
        input: &PathBuf,
        track_name: &str,
        output_dir: &PathBuf,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let output = output_dir.join(format!("{}.ogg", track_name));
        let quality = if self.config.quality.trim().is_empty() {
            "8"
        } else {
            &self.config.quality
        };

        // 1. Try 'oggenc' CLI
        if is_command_available("oggenc") {
            let res = Command::new("oggenc")
                .arg("-Q")
                .arg("-q")
                .arg(quality)
                .arg("-o")
                .arg(&output)
                .arg(input)
                .status();

            match res {
                Ok(status) if status.success() => return Ok(output),
                Ok(status) => {
                    eprintln!("oggenc returned exit code {:?}", status.code());
                }
                Err(e) => {
                    eprintln!("oggenc execution failed: {e}");
                }
            }
        }

        // 2. Fallback to 'ffmpeg'
        if is_command_available("ffmpeg") {
            let res = Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(input)
                .arg("-c:a")
                .arg("libvorbis")
                .arg("-q:a")
                .arg(quality)
                .arg(&output)
                .status();

            if let Ok(status) = res {
                if status.success() {
                    return Ok(output);
                }
            }
        }

        Err(
            "Ogg-Encoder ('oggenc' oder 'ffmpeg') nicht im PATH gefunden. Bitte vorbis-tools oder FFmpeg installieren oder FLAC/WAV wählen."
                .into(),
        )
    }

    fn encode_opus(
        &self,
        input: &PathBuf,
        track_name: &str,
        output_dir: &PathBuf,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let output = output_dir.join(format!("{}.opus", track_name));
        let bitrate = if self.config.bitrate.trim().is_empty() {
            "192"
        } else {
            &self.config.bitrate
        };

        // 1. Try 'opusenc' CLI
        if is_command_available("opusenc") {
            let mut cmd = Command::new("opusenc");
            cmd.arg("--bitrate").arg(bitrate);
            cmd.arg(input).arg(&output);

            let res = cmd.status();
            match res {
                Ok(status) if status.success() => return Ok(output),
                Ok(status) => {
                    eprintln!("opusenc returned exit code {:?}", status.code());
                }
                Err(e) => {
                    eprintln!("opusenc execution failed: {e}");
                }
            }
        }

        // 2. Fallback to 'ffmpeg'
        if is_command_available("ffmpeg") {
            let res = Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(input)
                .arg("-c:a")
                .arg("libopus")
                .arg("-b:a")
                .arg(format!("{}k", bitrate))
                .arg(&output)
                .status();

            if let Ok(status) = res {
                if status.success() {
                    return Ok(output);
                }
            }
        }

        Err(
            "Opus-Encoder ('opusenc' oder 'ffmpeg') nicht im PATH gefunden. Bitte opus-tools oder FFmpeg installieren (z. B. 'winget install Xiph.Org.Opus-Tools' oder 'winget install Gyan.FFmpeg') oder FLAC/WAV wählen."
                .into(),
        )
    }
}

/// Checks if an executable command is present in the system PATH or alongside the application executable.
pub fn is_command_available(cmd_name: &str) -> bool {
    #[cfg(windows)]
    {
        let check_cmd = format!("{cmd_name}.exe");
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                if dir.join(cmd_name).is_file() || dir.join(&check_cmd).is_file() {
                    return true;
                }
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                if parent.join(cmd_name).is_file() || parent.join(&check_cmd).is_file() {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                if dir.join(cmd_name).is_file() {
                    return true;
                }
            }
        }
        false
    }
}

/// Validates whether the required encoder tool for a target audio format is available on this system.
pub fn check_encoder_tool_available(encoder: &str) -> Result<(), String> {
    match encoder {
        "wav" | "flac" => Ok(()), // 100% Pure-Rust built-in
        "opus" => {
            if is_command_available("opusenc") || is_command_available("ffmpeg") {
                Ok(())
            } else {
                Err("Für Opus wird 'opusenc' (opus-tools) oder 'ffmpeg' benötigt.".to_string())
            }
        }
        "mp3" => {
            if is_command_available("lame") || is_command_available("ffmpeg") {
                Ok(())
            } else {
                Err("Für MP3 wird 'lame' oder 'ffmpeg' benötigt.".to_string())
            }
        }
        "ogg" => {
            if is_command_available("oggenc") || is_command_available("ffmpeg") {
                Ok(())
            } else {
                Err(
                    "Für Ogg Vorbis wird 'oggenc' (vorbis-tools) oder 'ffmpeg' benötigt."
                        .to_string(),
                )
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn sanitizes_invalid_filename_characters() {
        assert_eq!(
            sanitize_path_component("AC/DC: Live?  ", "Fallback"),
            "AC DC Live"
        );
    }

    #[test]
    fn falls_back_for_empty_or_reserved_names() {
        assert_eq!(sanitize_path_component("..", "Track 01"), "Track 01");
        assert!(is_windows_reserved_name("con"));
        assert_eq!(sanitize_track_name("CON", 7), "Track 07");
    }

    #[test]
    fn uses_final_name_as_working_wav_for_wav_output() {
        let output_dir = Path::new("/tmp/My Album");

        assert_eq!(
            working_wav_path("wav", 3, "Song Title", output_dir),
            output_dir.join("Song Title.wav")
        );
    }

    #[test]
    fn keeps_numbered_temp_wav_for_encoded_formats() {
        let output_dir = Path::new("/tmp/My Album");

        assert_eq!(
            working_wav_path("flac", 3, "Song Title", output_dir),
            output_dir.join("track03.wav")
        );
        assert_eq!(
            final_output_path("flac", "Song Title", output_dir),
            output_dir.join("Song Title.flac")
        );
        assert_eq!(
            working_wav_path("opus", 4, "Song Title", output_dir),
            output_dir.join("track04.wav")
        );
        assert_eq!(
            final_output_path("opus", "Song Title", output_dir),
            output_dir.join("Song Title.opus")
        );
    }

    #[test]
    fn supports_all_target_audio_formats() {
        let output_dir = Path::new("/tmp/Music");
        let formats = [
            ("flac", "flac"),
            ("mp3", "mp3"),
            ("ogg", "ogg"),
            ("opus", "opus"),
            ("wav", "wav"),
        ];

        for (fmt, ext) in formats {
            assert_eq!(
                final_output_path(fmt, "Test Track", output_dir),
                output_dir.join(format!("Test Track.{}", ext))
            );
        }
    }
}
