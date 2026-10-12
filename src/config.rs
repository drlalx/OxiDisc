use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub encoder: String,
    pub bitrate: String,
    pub quality: String,
    pub device: String,
    pub ui_backend: String,                  // egui | tui
    pub language: String, // auto | en | de | fr | it | es | pt | ja | ru | pl | zh
    pub metadata_source: String, // musicbrainz
    pub album_art_size_preference: String, // auto | small | large | original
    pub album_art_download_behavior: String, // preview-only | save-with-rip
    pub auto_create_output_dir: bool, // false = ask user before creating missing directories
}

pub fn default_platform_device() -> String {
    #[cfg(windows)]
    {
        crate::windows_cd::detect_windows_cd_drives()
            .first()
            .cloned()
            .unwrap_or_else(|| "D:".to_string())
    }
    #[cfg(not(windows))]
    {
        "/dev/sr0".to_string()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            encoder: "flac".to_string(),
            bitrate: "320".to_string(),
            quality: "8".to_string(),
            device: default_platform_device(),
            ui_backend: "egui".to_string(),
            language: "auto".to_string(),
            // Default to Auto to query MusicBrainz then GnuDB automatically
            metadata_source: "auto".to_string(),
            album_art_size_preference: "auto".to_string(),
            album_art_download_behavior: "preview-only".to_string(),
            auto_create_output_dir: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            let cfg = Self::default();
            let _ = cfg.save();
            cfg
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        fs::write(config_path, content)
    }

    fn config_path() -> PathBuf {
        if let Ok(path) = std::env::var("OXIDISC_CONFIG") {
            return PathBuf::from(path);
        }

        if let Ok(cwd) = std::env::current_dir() {
            let repo_config = cwd.join("config").join("config.toml");
            if repo_config.exists() {
                return repo_config;
            }
        }

        let base_config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base_config_dir.join("oxidisc").join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.encoder, "flac");
        assert_eq!(cfg.bitrate, "320");
        assert_eq!(cfg.ui_backend, "egui");
        assert_eq!(cfg.language, "auto");
        assert_eq!(cfg.metadata_source, "auto");
        assert_eq!(cfg.auto_create_output_dir, false);
        assert!(!cfg.device.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let cfg = Config::default();
        let serialized = toml::to_string_pretty(&cfg).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(cfg, deserialized);
    }
}
