#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod acoustid;
mod cd_reader;
mod config;
mod discid;
#[cfg(feature = "egui-ui")]
mod egui_app;
mod i18n;
pub mod mock_cd;
mod ripper;
#[cfg(feature = "tui")]
mod tui_app;
mod wav;
#[cfg(windows)]
mod windows_cd;

use config::Config;
use std::fmt;

#[cfg(not(any(feature = "egui-ui", feature = "tui")))]
compile_error!("Enable at least one UI feature: egui-ui or tui.");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiBackend {
    Egui,
    Tui,
}

impl UiBackend {
    fn parse(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "egui" | "egui-ui" => Some(Self::Egui),
            "tui" | "tui-ui" | "terminal" | "ratatui" => Some(Self::Tui),
            _ => None,
        }
    }

    fn is_compiled(self) -> bool {
        match self {
            Self::Egui => cfg!(feature = "egui-ui"),
            Self::Tui => cfg!(feature = "tui"),
        }
    }

    fn as_config_value(self) -> &'static str {
        match self {
            Self::Egui => "egui",
            Self::Tui => "tui",
        }
    }
}

impl fmt::Display for UiBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_value())
    }
}

fn default_compiled_ui_backend() -> UiBackend {
    #[cfg(feature = "egui-ui")]
    {
        UiBackend::Egui
    }
    #[cfg(all(not(feature = "egui-ui"), feature = "tui"))]
    {
        UiBackend::Tui
    }
}

fn print_compiled_ui_backend() {
    let mut backends = Vec::new();
    #[cfg(feature = "egui-ui")]
    backends.push("egui-ui");
    #[cfg(feature = "tui")]
    backends.push("tui");

    println!("Compiled UI backend(s): {}", backends.join(", "));
}

fn print_usage() {
    println!(
        "Usage: oxidisc [OPTIONS]\n\nOptions:\n  --tui                    Launch Terminal UI (Ratatui)\n  --ui <egui|tui>          Select active UI backend (saved for future launches)\n  --feature <backend>      Alias for --ui\n  --mock-cd, --mock        Run with virtual/synthetic test CD data (hardware-free mode)\n  --generate-test-disc [dir] Generate CUE/BIN audio CD test image files on disk\n  --device <path>, -d      Specify/override optical drive device path\n  --help, -h               Show this help message\n\nWith cargo run, pass runtime options after --, for example:\n  cargo run -- --ui egui"
    );
}

fn selected_ui_backend() -> Result<Option<UiBackend>, String> {
    let mut args = std::env::args().skip(1);
    let mut selected = None;

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            print_usage();
            std::process::exit(0);
        }

        if arg == "--generate-test-disc" {
            let out_dir = args
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("test_data"));
            println!(
                "Generating synthetic Audio CD image in: {}",
                out_dir.display()
            );
            match mock_cd::generate_test_disc_files(&out_dir, false) {
                Ok((cue, bin)) => {
                    println!(
                        "Successfully generated test disc files:\n  CUE: {}\n  BIN: {}",
                        cue.display(),
                        bin.display()
                    );
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("Error generating test disc: {err}");
                    std::process::exit(1);
                }
            }
        }

        if arg == "--mock-cd" || arg == "--mock" || arg == "--test" {
            unsafe {
                std::env::set_var("CD_DEVICE", "mock");
            }
            println!("Note: Running in Mock CD / Hardware-free Test Mode.");
            continue;
        }

        if arg == "--device" || arg == "-d" {
            let dev = args
                .next()
                .ok_or_else(|| "--device requires a device path argument".to_string())?;
            unsafe {
                std::env::set_var("CD_DEVICE", &dev);
            }
            continue;
        }

        if let Some(dev) = arg.strip_prefix("--device=") {
            unsafe {
                std::env::set_var("CD_DEVICE", dev);
            }
            continue;
        }

        if arg == "--tui" {
            let backend = UiBackend::Tui;
            if !backend.is_compiled() {
                return Err("UI backend 'tui' is not compiled into this binary. Rebuild with --features tui.".to_string());
            }
            selected = Some(backend);
            continue;
        }

        let value = if arg == "--ui" || arg == "--feature" {
            args.next()
                .ok_or_else(|| format!("{arg} requires one of: egui, tui"))?
        } else if let Some(value) = arg.strip_prefix("--ui=") {
            value.to_string()
        } else if let Some(value) = arg.strip_prefix("--feature=") {
            value.to_string()
        } else {
            return Err(format!("Unrecognized argument: {arg}"));
        };

        let backend = UiBackend::parse(&value)
            .ok_or_else(|| format!("Unsupported UI backend '{value}'. Use egui or tui."))?;
        if !backend.is_compiled() {
            return Err(format!(
                "UI backend '{backend}' is not compiled into this binary. Rebuild with --features {}.",
                match backend {
                    UiBackend::Egui => "egui-ui",
                    UiBackend::Tui => "tui",
                }
            ));
        }
        selected = Some(backend);
    }

    Ok(selected)
}

fn resolve_ui_backend() -> Result<UiBackend, String> {
    if let Some(backend) = selected_ui_backend()? {
        let mut config = Config::load();
        config.ui_backend = backend.as_config_value().to_string();
        let _ = config.save();
        return Ok(backend);
    }

    let config = Config::load();
    Ok(UiBackend::parse(&config.ui_backend)
        .filter(|backend| backend.is_compiled())
        .unwrap_or_else(default_compiled_ui_backend))
}

#[cfg(feature = "egui-ui")]
fn run_egui_ui() -> eframe::Result<()> {
    print_compiled_ui_backend();
    egui_app::run()
}

#[cfg(feature = "tui")]
fn run_tui_ui() -> Result<(), Box<dyn std::error::Error>> {
    print_compiled_ui_backend();
    tui_app::run()
}

fn main() {
    match resolve_ui_backend() {
        #[cfg(feature = "egui-ui")]
        Ok(UiBackend::Egui) => {
            if let Err(err) = run_egui_ui() {
                eprintln!("Error running egui UI: {err}");
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "egui-ui"))]
        Ok(UiBackend::Egui) => unreachable!("egui-ui is not compiled"),
        #[cfg(feature = "tui")]
        Ok(UiBackend::Tui) => {
            if let Err(err) = run_tui_ui() {
                eprintln!("Error running TUI: {err}");
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "tui"))]
        Ok(UiBackend::Tui) => unreachable!("tui is not compiled"),
        Err(err) => {
            eprintln!("{err}");
            print_usage();
            std::process::exit(1);
        }
    }
}
