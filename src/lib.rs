#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod acoustid;
pub mod cd_reader;
pub mod config;
pub mod discid;
#[cfg(feature = "egui-ui")]
pub mod egui_app;
pub mod i18n;
pub mod mock_cd;
pub mod ripper;
#[cfg(feature = "tui")]
pub mod tui_app;
pub mod wav;
#[cfg(windows)]
pub mod windows_cd;
