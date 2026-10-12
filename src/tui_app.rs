use crate::cd_reader::{CdInfo, CdReader};
use crate::config::Config;
use crate::ripper::{RipMessage, Ripper};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Gauge, HighlightSpacing, Paragraph, Row, Table,
        TableState, Tabs,
    },
};
use std::io;
use std::path::PathBuf;
#[cfg(not(windows))]
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver};

// UI Color Palette matching the modern egui theme
const COLOR_CARD_BG: Color = Color::Rgb(22, 29, 41);
const COLOR_CARD_BG_ALT: Color = Color::Rgb(26, 35, 50);
const COLOR_BORDER: Color = Color::Rgb(42, 54, 72);
const COLOR_BORDER_FOCUS: Color = Color::Rgb(0, 230, 210);
const COLOR_ACCENT: Color = Color::Rgb(0, 230, 210);
const COLOR_ACCENT_DIM: Color = Color::Rgb(0, 160, 145);
const COLOR_TEXT_PRIMARY: Color = Color::Rgb(230, 238, 248);
const COLOR_TEXT_MUTED: Color = Color::Rgb(140, 155, 175);
const COLOR_TEXT_DIM: Color = Color::Rgb(90, 105, 125);
const COLOR_SUCCESS: Color = Color::Rgb(34, 197, 94);
const COLOR_DANGER: Color = Color::Rgb(239, 68, 68);

const FORMATS: &[(&str, &str)] = &[
    ("FLAC", "flac"),
    ("Opus", "opus"),
    ("MP3", "mp3"),
    ("WAV", "wav"),
    ("Ogg", "ogg"),
];

const BITRATES: &[(&str, &str)] = &[
    ("128k", "128"),
    ("192k", "192"),
    ("256k", "256"),
    ("320k", "320"),
];

const METADATA_SOURCES: &[(&str, &str)] = &[
    ("Auto", "auto"),
    ("MusicBrainz", "musicbrainz"),
    ("GnuDB", "gnudb"),
    ("AcoustID", "acoustid"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum TuiTab {
    Rip,
    Settings,
    Help,
}

pub struct OxiDiscTuiApp {
    active_tab: TuiTab,
    cd_info: Option<CdInfo>,
    pub language_index: usize,
    pub lang: crate::i18n::Language,
    format_index: usize,
    bitrate_index: usize,
    metadata_source_index: usize,
    output_dir: String,
    auto_create_output_dir: bool,
    is_tray_ejected: bool,
    selected_tracks: Vec<bool>,
    completed_tracks: Vec<bool>,
    current_ripping_track: Option<usize>,
    track_progress_fraction: f64,
    overall_progress_fraction: f64,
    progress_label: String,
    is_ripping: bool,
    rip_start_time: Option<Instant>,
    ripper: Option<Arc<Ripper>>,
    rip_receiver: Option<UnboundedReceiver<RipMessage>>,
    next_auto_detect_at: Instant,
    status: String,
    table_state: TableState,
    focus_sidebar: bool,
    settings_cursor: usize,
    editing_output_dir: bool,
    temp_output_dir: String,
    should_quit: bool,
}

impl Default for OxiDiscTuiApp {
    fn default() -> Self {
        let cfg = Config::load();
        let (language_index, lang) = match cfg.language.to_lowercase().as_str() {
            "en" => (1, crate::i18n::Language::En),
            "de" => (2, crate::i18n::Language::De),
            "fr" => (3, crate::i18n::Language::Fr),
            "it" => (4, crate::i18n::Language::It),
            "es" => (5, crate::i18n::Language::Es),
            "pt" => (6, crate::i18n::Language::Pt),
            "ja" => (7, crate::i18n::Language::Ja),
            "ru" => (8, crate::i18n::Language::Ru),
            "pl" => (9, crate::i18n::Language::Pl),
            "zh" => (10, crate::i18n::Language::Zh),
            _ => (0, crate::i18n::Language::resolve(&cfg.language)),
        };
        let format_index = match cfg.encoder.as_str() {
            "opus" => 1,
            "mp3" => 2,
            "wav" => 3,
            "ogg" => 4,
            _ => 0, // FLAC
        };
        let bitrate_index = match cfg.bitrate.as_str() {
            "128" => 0,
            "192" => 1,
            "256" => 2,
            _ => 3, // 320 kbps
        };
        let metadata_source_index = match cfg.metadata_source.as_str() {
            "musicbrainz" => 1,
            "gnudb" => 2,
            "acoustid" => 3,
            _ => 0, // Auto
        };
        let output_dir = std::env::var("OXIDISC_OUTPUT_DIR")
            .ok()
            .or_else(|| dirs::audio_dir().map(|p| p.to_string_lossy().to_string()))
            .or_else(|| {
                dirs::home_dir().map(|home| home.join("Music").to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "~/Music".to_string());

        let mut app = Self {
            active_tab: TuiTab::Rip,
            cd_info: None,
            language_index,
            lang,
            format_index,
            bitrate_index,
            metadata_source_index,
            output_dir,
            auto_create_output_dir: cfg.auto_create_output_dir,
            is_tray_ejected: false,
            selected_tracks: Vec::new(),
            completed_tracks: Vec::new(),
            current_ripping_track: None,
            track_progress_fraction: 0.0,
            overall_progress_fraction: 0.0,
            progress_label: String::new(),
            is_ripping: false,
            rip_start_time: None,
            ripper: None,
            rip_receiver: None,
            next_auto_detect_at: Instant::now(),
            status: "Bereit. Suche nach Audio-CD...".to_string(),
            table_state: TableState::default(),
            focus_sidebar: false,
            settings_cursor: 0,
            editing_output_dir: false,
            temp_output_dir: String::new(),
            should_quit: false,
        };

        app.detect_cd();
        app
    }
}

impl OxiDiscTuiApp {
    const AUTO_DETECT_RETRY_INTERVAL: Duration = Duration::from_secs(3);

    pub fn update_language_from_index(&mut self) {
        self.lang = match self.language_index {
            1 => crate::i18n::Language::En,
            2 => crate::i18n::Language::De,
            3 => crate::i18n::Language::Fr,
            4 => crate::i18n::Language::It,
            5 => crate::i18n::Language::Es,
            6 => crate::i18n::Language::Pt,
            7 => crate::i18n::Language::Ja,
            8 => crate::i18n::Language::Ru,
            9 => crate::i18n::Language::Pl,
            10 => crate::i18n::Language::Zh,
            _ => crate::i18n::Language::resolve("auto"),
        };
    }

    pub fn language_key(&self) -> &'static str {
        match self.language_index {
            1 => "en",
            2 => "de",
            3 => "fr",
            4 => "it",
            5 => "es",
            6 => "pt",
            7 => "ja",
            8 => "ru",
            9 => "pl",
            10 => "zh",
            _ => "auto",
        }
    }

    pub fn cycle_language(&mut self, forward: bool) {
        if forward {
            self.language_index = (self.language_index + 1) % 11;
        } else if self.language_index == 0 {
            self.language_index = 10;
        } else {
            self.language_index -= 1;
        }
        self.update_language_from_index();
    }

    pub fn encoder_key(&self) -> &'static str {
        FORMATS
            .get(self.format_index)
            .map(|(_, k)| *k)
            .unwrap_or("flac")
    }

    pub fn bitrate_key(&self) -> &'static str {
        BITRATES
            .get(self.bitrate_index)
            .map(|(_, k)| *k)
            .unwrap_or("320")
    }

    pub fn metadata_source_key(&self) -> &'static str {
        METADATA_SOURCES
            .get(self.metadata_source_index)
            .map(|(_, k)| *k)
            .unwrap_or("auto")
    }

    pub fn detect_cd(&mut self) {
        if self.is_ripping {
            return;
        }

        let source = self.metadata_source_key();
        match CdReader::detect_with_metadata_source(source) {
            Ok(cd_info) => {
                let count = cd_info.tracks.len();
                self.status = if let Some(err) = &cd_info.metadata_error {
                    format!("{}. {err}", self.lang.detected_tracks(count))
                } else if let Some(provider) = &cd_info.metadata_provider {
                    self.lang.detected_tracks_with_provider(count, provider)
                } else {
                    self.lang.detected_tracks(count)
                };
                self.selected_tracks = vec![true; count];
                self.completed_tracks = vec![false; count];
                self.cd_info = Some(cd_info);
                self.is_tray_ejected = false;
                if count > 0 && self.table_state.selected().is_none() {
                    self.table_state.select(Some(0));
                }
            }
            Err(err) => {
                self.status = format!("Keine CD erkannt ({err})");
                self.selected_tracks.clear();
                self.completed_tracks.clear();
                self.cd_info = None;
                self.table_state.select(None);
            }
        }
    }

    pub fn maybe_auto_detect_cd(&mut self) {
        if self.cd_info.is_some() || self.is_ripping || self.is_tray_ejected {
            return;
        }

        let now = Instant::now();
        if now >= self.next_auto_detect_at {
            self.next_auto_detect_at = now + Self::AUTO_DETECT_RETRY_INTERVAL;
            self.detect_cd();
        }
    }

    pub fn eject_disc(&mut self) {
        if self.is_ripping {
            self.status = "Auswerfen während des Rippens nicht möglich!".to_string();
            return;
        }

        #[cfg(windows)]
        {
            let dev = CdReader::active_device_path();
            match crate::windows_cd::eject_windows_drive(&dev) {
                Ok(()) => {
                    self.status = "CD ausgeworfen (Laufwerk geöffnet).".to_string();
                    self.cd_info = None;
                    self.selected_tracks.clear();
                    self.completed_tracks.clear();
                    self.is_tray_ejected = true;
                    self.next_auto_detect_at = Instant::now() + Duration::from_secs(5);
                    self.table_state.select(None);
                }
                Err(err) => {
                    self.status = format!("Auswerfen fehlgeschlagen: {err}");
                }
            }
        }

        #[cfg(not(windows))]
        {
            let dev = CdReader::active_device_path();
            let mut cmd = Command::new("eject");
            if !crate::mock_cd::is_mock_device(&dev) {
                cmd.arg(&dev);
            }
            match cmd.status() {
                Ok(status) if status.success() => {
                    self.status = "CD ausgeworfen (Laufwerk geöffnet).".to_string();
                    self.cd_info = None;
                    self.selected_tracks.clear();
                    self.completed_tracks.clear();
                    self.is_tray_ejected = true;
                    self.next_auto_detect_at = Instant::now() + Duration::from_secs(5);
                    self.table_state.select(None);
                }
                Ok(_) => {
                    self.status = "Eject-Befehl fehlgeschlagen. Bitte sicherstellen, dass 'eject' installiert ist.".to_string();
                }
                Err(err) => {
                    self.status = format!("Konnte 'eject' nicht ausführen: {err}");
                }
            }
        }
    }

    pub fn load_disc(&mut self) {
        if self.is_ripping {
            return;
        }

        #[cfg(windows)]
        {
            let dev = CdReader::active_device_path();
            match crate::windows_cd::load_windows_drive(&dev) {
                Ok(()) => {
                    self.status = "Laufwerk wird eingezogen. Lese Disc ein...".to_string();
                    self.is_tray_ejected = false;
                    self.next_auto_detect_at = Instant::now();
                    self.detect_cd();
                }
                Err(err) => {
                    self.status = format!("Laufwerk konnte nicht geschlossen werden: {err}");
                    self.is_tray_ejected = false;
                }
            }
        }

        #[cfg(not(windows))]
        {
            let dev = CdReader::active_device_path();
            let mut cmd = Command::new("eject");
            cmd.arg("-t");
            if !crate::mock_cd::is_mock_device(&dev) {
                cmd.arg(&dev);
            }
            match cmd.status() {
                Ok(status) if status.success() => {
                    self.status = "Laufwerk wird eingezogen. Lese Disc ein...".to_string();
                    self.is_tray_ejected = false;
                    self.next_auto_detect_at = Instant::now();
                    self.detect_cd();
                }
                Ok(_) => {
                    self.status = "Befehl 'eject -t' fehlgeschlagen.".to_string();
                    self.is_tray_ejected = false;
                }
                Err(err) => {
                    self.status = format!("Konnte 'eject -t' nicht ausführen: {err}");
                    self.is_tray_ejected = false;
                }
            }
        }
    }

    pub fn start_ripping(&mut self) {
        if self.is_ripping {
            return;
        }

        let Some(cd_info) = &self.cd_info else {
            self.status = "Keine CD erkannt zum Rippen!".to_string();
            return;
        };

        let selected_indices: Vec<usize> = self
            .selected_tracks
            .iter()
            .enumerate()
            .filter_map(|(i, &sel)| sel.then_some(i))
            .collect();

        if selected_indices.is_empty() {
            self.status = "Bitte mindestens einen Track auswählen!".to_string();
            return;
        }

        let enc_key = self.encoder_key();
        if let Err(err) = crate::ripper::check_encoder_tool_available(enc_key) {
            self.status = format!("Encoder-Fehler: {err}");
            return;
        }

        let out_path = PathBuf::from(self.output_dir.trim());
        if !out_path.exists() {
            if let Err(e) = std::fs::create_dir_all(&out_path) {
                self.status = format!("Konnte Ausgabe-Ordner nicht erstellen: {e}");
                return;
            }
        }

        let mut cfg = Config::load();
        cfg.language = self.language_key().to_string();
        cfg.encoder = enc_key.to_string();
        cfg.bitrate = self.bitrate_key().to_string();
        cfg.metadata_source = self.metadata_source_key().to_string();
        cfg.auto_create_output_dir = self.auto_create_output_dir;
        let _ = cfg.save();

        let mut rip_cd = cd_info.clone();
        rip_cd.tracks = rip_cd
            .tracks
            .into_iter()
            .enumerate()
            .filter_map(|(i, t)| {
                self.selected_tracks
                    .get(i)
                    .copied()
                    .unwrap_or(false)
                    .then_some(t)
            })
            .collect();

        let (sender, receiver) = mpsc::unbounded_channel();
        let ripper = match Ripper::new(cfg, out_path, sender) {
            Ok(r) => Arc::new(r),
            Err(e) => {
                self.status = format!("Fehler beim Initialisieren des Rippers: {e}");
                return;
            }
        };

        self.overall_progress_fraction = 0.0;
        self.track_progress_fraction = 0.0;
        self.current_ripping_track = Some(1);
        self.completed_tracks = vec![false; self.selected_tracks.len()];
        self.progress_label = "Starte Ripping-Vorgang...".to_string();
        self.status = "Rippen gestartet...".to_string();
        self.rip_start_time = Some(Instant::now());
        self.is_ripping = true;
        self.ripper = Some(ripper.clone());
        self.rip_receiver = Some(receiver);

        std::thread::spawn(move || {
            ripper.rip(&rip_cd);
        });
    }

    pub fn stop_ripping(&mut self) {
        if let Some(ripper) = &self.ripper {
            ripper.cancel();
        }
        self.is_ripping = false;
        self.ripper = None;
        self.rip_receiver = None;
        self.current_ripping_track = None;
        self.overall_progress_fraction = 0.0;
        self.track_progress_fraction = 0.0;
        self.progress_label.clear();
        self.status = "Rippen abgebrochen.".to_string();
    }

    pub fn poll_ripper_messages(&mut self) {
        let mut disconnected = false;
        let mut messages = Vec::new();

        if let Some(receiver) = self.rip_receiver.as_mut() {
            loop {
                match receiver.try_recv() {
                    Ok(msg) => messages.push(msg),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        for msg in messages {
            match msg {
                RipMessage::Progress(overall, label) => {
                    self.overall_progress_fraction = overall.clamp(0.0, 1.0);
                    self.progress_label = label.clone();

                    if let Some(track_idx) = parse_track_index_from_label(&label) {
                        self.current_ripping_track = Some(track_idx);
                    }
                    if let Some(pct) = parse_percent_from_label(&label) {
                        self.track_progress_fraction = (pct / 100.0).clamp(0.0, 1.0);
                    }
                }
                RipMessage::TrackComplete(track_idx) => {
                    if track_idx > 0 && track_idx <= self.completed_tracks.len() {
                        self.completed_tracks[track_idx - 1] = true;
                    }
                }
                RipMessage::Success => {
                    self.is_ripping = false;
                    self.ripper = None;
                    self.rip_receiver = None;
                    self.current_ripping_track = None;
                    self.overall_progress_fraction = 1.0;
                    self.track_progress_fraction = 1.0;
                    self.progress_label = "Fertig!".to_string();
                    self.status = "✔ Audio-CD erfolgreich extrahiert!".to_string();
                }
                RipMessage::Error(err) => {
                    self.is_ripping = false;
                    self.ripper = None;
                    self.rip_receiver = None;
                    self.current_ripping_track = None;
                    self.status = format!("Ripping-Fehler: {err}");
                }
            }
        }

        if disconnected && self.is_ripping {
            self.is_ripping = false;
            self.ripper = None;
            self.rip_receiver = None;
            self.current_ripping_track = None;
        }
    }

    pub fn toggle_current_track(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(state) = self.selected_tracks.get_mut(selected) {
                *state = !*state;
            }
        }
    }

    pub fn select_all_tracks(&mut self, select: bool) {
        for s in &mut self.selected_tracks {
            *s = select;
        }
    }

    pub fn next_track(&mut self) {
        let total = self.selected_tracks.len();
        if total == 0 {
            return;
        }
        let next = match self.table_state.selected() {
            Some(i) => (i + 1).min(total - 1),
            None => 0,
        };
        self.table_state.select(Some(next));
    }

    pub fn prev_track(&mut self) {
        let total = self.selected_tracks.len();
        if total == 0 {
            return;
        }
        let prev = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.table_state.select(Some(prev));
    }

    pub fn cycle_format(&mut self, forward: bool) {
        if forward {
            self.format_index = (self.format_index + 1) % FORMATS.len();
        } else {
            self.format_index = if self.format_index == 0 {
                FORMATS.len() - 1
            } else {
                self.format_index - 1
            };
        }
    }

    pub fn cycle_bitrate(&mut self, forward: bool) {
        if forward {
            self.bitrate_index = (self.bitrate_index + 1) % BITRATES.len();
        } else {
            self.bitrate_index = if self.bitrate_index == 0 {
                BITRATES.len() - 1
            } else {
                self.bitrate_index - 1
            };
        }
    }
}

fn parse_track_index_from_label(label: &str) -> Option<usize> {
    let lower = label.to_lowercase();
    let marker = "track ";
    let idx = lower.find(marker)?;
    let remainder = &lower[idx + marker.len()..];
    let num_str: String = remainder
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num_str.parse().ok()
}

fn parse_percent_from_label(label: &str) -> Option<f64> {
    let open_paren = label.rfind('(')?;
    let close_paren = label.rfind(')')?;
    if close_paren <= open_paren {
        return None;
    }
    let inner = &label[open_paren + 1..close_paren];
    let pct_str = inner.trim_end_matches('%').trim();
    pct_str.parse().ok()
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Setup clean terminal with panic hook protection
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    let mut app = OxiDiscTuiApp::default();
    let res = run_event_loop(&mut terminal, &mut app);

    // Restore terminal cleanly
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("TUI Runtime Error: {err:?}");
    }

    Ok(())
}

fn run_event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut OxiDiscTuiApp,
) -> io::Result<()> {
    loop {
        app.poll_ripper_messages();
        app.maybe_auto_detect_cd();

        terminal
            .draw(|f| render_ui(f, app))
            .map_err(|e| io::Error::other(e.to_string()))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(60))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(app, key);
                }
            }
        }
    }
    Ok(())
}

fn handle_key_event(app: &mut OxiDiscTuiApp, key: event::KeyEvent) {
    if app.editing_output_dir {
        match key.code {
            KeyCode::Enter => {
                if !app.temp_output_dir.trim().is_empty() {
                    app.output_dir = app.temp_output_dir.trim().to_string();
                }
                app.editing_output_dir = false;
            }
            KeyCode::Esc => {
                app.editing_output_dir = false;
            }
            KeyCode::Backspace => {
                app.temp_output_dir.pop();
            }
            KeyCode::Char(c) => {
                app.temp_output_dir.push(c);
            }
            _ => {}
        }
        return;
    }

    // Global Hotkeys
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), KeyModifiers::NONE) => {
            if app.is_ripping {
                app.stop_ripping();
            }
            app.should_quit = true;
            return;
        }
        (KeyCode::Char('1'), KeyModifiers::NONE) => {
            app.active_tab = TuiTab::Rip;
            return;
        }
        (KeyCode::Char('2'), KeyModifiers::NONE) => {
            app.active_tab = TuiTab::Settings;
            return;
        }
        (KeyCode::Char('3'), KeyModifiers::NONE) => {
            app.active_tab = TuiTab::Help;
            return;
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            app.focus_sidebar = !app.focus_sidebar;
            return;
        }
        _ => {}
    }

    // Tab-specific handlers
    match app.active_tab {
        TuiTab::Rip => handle_rip_tab_keys(app, key),
        TuiTab::Settings => handle_settings_tab_keys(app, key),
        TuiTab::Help => {
            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                app.active_tab = TuiTab::Rip;
            }
        }
    }
}

fn handle_rip_tab_keys(app: &mut OxiDiscTuiApp, key: event::KeyEvent) {
    match key.code {
        KeyCode::Char('r') => {
            if app.is_ripping {
                app.stop_ripping();
            } else {
                app.start_ripping();
            }
        }
        KeyCode::Char('c') => {
            if app.is_ripping {
                app.stop_ripping();
            }
        }
        KeyCode::Char('e') => {
            app.eject_disc();
        }
        KeyCode::Char('l') => {
            app.load_disc();
        }
        KeyCode::Char('u') => {
            app.detect_cd();
        }
        KeyCode::Char('f') => {
            app.cycle_format(true);
        }
        KeyCode::Char('F') => {
            app.cycle_format(false);
        }
        KeyCode::Char('b') => {
            app.cycle_bitrate(true);
        }
        KeyCode::Char('B') => {
            app.cycle_bitrate(false);
        }
        KeyCode::Char('o') => {
            app.temp_output_dir = app.output_dir.clone();
            app.editing_output_dir = true;
        }
        KeyCode::Char(' ') => {
            app.toggle_current_track();
        }
        KeyCode::Char('a') => {
            app.select_all_tracks(true);
        }
        KeyCode::Char('n') => {
            app.select_all_tracks(false);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.prev_track();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_track();
        }
        KeyCode::Home => {
            if !app.selected_tracks.is_empty() {
                app.table_state.select(Some(0));
            }
        }
        KeyCode::End => {
            if !app.selected_tracks.is_empty() {
                app.table_state.select(Some(app.selected_tracks.len() - 1));
            }
        }
        _ => {}
    }
}

fn handle_settings_tab_keys(app: &mut OxiDiscTuiApp, key: event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings_cursor = app.settings_cursor.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.settings_cursor = (app.settings_cursor + 1).min(5);
        }
        KeyCode::Left | KeyCode::Char('h') => match app.settings_cursor {
            0 => app.cycle_language(false),
            1 => {
                app.metadata_source_index = if app.metadata_source_index == 0 {
                    METADATA_SOURCES.len() - 1
                } else {
                    app.metadata_source_index - 1
                };
            }
            2 => app.cycle_format(false),
            3 => app.cycle_bitrate(false),
            4 => app.auto_create_output_dir = !app.auto_create_output_dir,
            _ => {}
        },
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
            match app.settings_cursor {
                0 => app.cycle_language(true),
                1 => {
                    app.metadata_source_index =
                        (app.metadata_source_index + 1) % METADATA_SOURCES.len();
                }
                2 => app.cycle_format(true),
                3 => app.cycle_bitrate(true),
                4 => app.auto_create_output_dir = !app.auto_create_output_dir,
                5 => {
                    app.temp_output_dir = app.output_dir.clone();
                    app.editing_output_dir = true;
                }
                _ => {}
            }
        }
        KeyCode::Char('s') => {
            let mut cfg = Config::load();
            cfg.language = app.language_key().to_string();
            cfg.metadata_source = app.metadata_source_key().to_string();
            cfg.encoder = app.encoder_key().to_string();
            cfg.bitrate = app.bitrate_key().to_string();
            cfg.auto_create_output_dir = app.auto_create_output_dir;
            cfg.ui_backend = "tui".to_string();
            let _ = cfg.save();
            app.status = format!("✔ {}", app.lang.settings_saved());
        }
        _ => {}
    }
}

// UI RENDERING

fn render_ui(f: &mut Frame, app: &mut OxiDiscTuiApp) {
    let size = f.area();

    // Top-level vertical split: Header, Main Area, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Status & Shortcuts Bar
        ])
        .split(size);

    render_header(f, app, chunks[0]);

    match app.active_tab {
        TuiTab::Rip => render_rip_view(f, app, chunks[1]),
        TuiTab::Settings => render_settings_view(f, app, chunks[1]),
        TuiTab::Help => render_help_view(f, app, chunks[1]),
    }

    render_footer(f, app, chunks[2]);

    if app.editing_output_dir {
        render_output_dir_modal(f, app, size);
    }
}

fn render_header(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22), // App Title
            Constraint::Length(38), // Tabs
            Constraint::Min(20),    // Drive badge & Tray actions
        ])
        .split(area);

    // Title Block
    let title = Paragraph::new("💿 OxiDisc")
        .style(
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER)),
        );
    f.render_widget(title, chunks[0]);

    // Navigation Tabs
    let tab_titles = vec![
        format!("[1] {}", app.lang.tab_rip()),
        format!("[2] {}", app.lang.tab_settings()),
        format!("[3] {}", app.lang.tab_help()),
    ];
    let tab_idx = match app.active_tab {
        TuiTab::Rip => 0,
        TuiTab::Settings => 1,
        TuiTab::Help => 2,
    };
    let tabs = Tabs::new(tab_titles)
        .select(tab_idx)
        .style(Style::default().fg(COLOR_TEXT_MUTED))
        .highlight_style(
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER)),
        );
    f.render_widget(tabs, chunks[1]);

    // Drive status & Quick Actions (Auswerfen / Einziehen / Aktualisieren)
    let dev = CdReader::active_device_path();
    let dev_label = if crate::mock_cd::is_mock_device(&dev) {
        "Mock Virtual CD".to_string()
    } else {
        dev
    };

    let eject_label = if app.is_tray_ejected {
        format!("[l] 📥 {}", app.lang.load_tray())
    } else {
        format!("[e] ⏏ {}", app.lang.eject())
    };

    let drive_info = Line::from(vec![
        Span::styled(
            format!("{}: ", app.lang.drive()),
            Style::default().fg(COLOR_TEXT_MUTED),
        ),
        Span::styled(
            format!("[{dev_label}]  "),
            Style::default().fg(Color::Rgb(140, 165, 195)).bold(),
        ),
        Span::styled(
            format!("{eject_label}  "),
            Style::default().fg(COLOR_ACCENT_DIM).bold(),
        ),
        Span::styled(
            format!("[u] 🔄 {}", app.lang.refresh()),
            Style::default().fg(COLOR_TEXT_MUTED).bold(),
        ),
    ]);

    let drive_block = Paragraph::new(drive_info)
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER)),
        );
    f.render_widget(drive_block, chunks[2]);
}

fn render_rip_view(f: &mut Frame, app: &mut OxiDiscTuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(50), // Left Sidebar
            Constraint::Min(30),    // Tracklist Table
        ])
        .split(area);

    render_sidebar(f, app, chunks[0]);
    render_tracklist(f, app, chunks[1]);
}

fn render_sidebar(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Album & Disc Info with ASCII art
            Constraint::Length(6),  // Output format & Bitrate pills
            Constraint::Length(4),  // Output Directory
            Constraint::Min(6),     // Ripper action button & progress
        ])
        .split(area);

    // 1. Album Info Block
    render_album_art_block(f, app, sidebar_chunks[0]);

    // 2. Format & Bitrate Block
    render_format_block(f, app, sidebar_chunks[1]);

    // 3. Output Directory Block
    render_output_dir_block(f, app, sidebar_chunks[2]);

    // 4. Rip Control & Progress Block
    render_rip_control_block(f, app, sidebar_chunks[3]);
}

fn render_album_art_block(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let block = Block::default()
        .title(format!(" 💿 {} ", app.lang.disc_info()))
        .title_style(Style::default().fg(COLOR_TEXT_MUTED).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(17), // ASCII Disc Graphic
            Constraint::Min(10),    // Metadata info
        ])
        .split(inner);

    let has_cd = app.cd_info.is_some();
    let ascii_lines = render_vinyl_ascii(has_cd, app.is_ripping);
    let ascii_p = Paragraph::new(ascii_lines);
    f.render_widget(ascii_p, content_split[0]);

    // Metadata details
    let mut info_lines = Vec::new();
    if let Some(cd) = &app.cd_info {
        info_lines.push(Line::from(Span::styled(
            truncate_str(&cd.title, 18),
            Style::default().fg(COLOR_ACCENT).bold(),
        )));
        info_lines.push(Line::from(Span::styled(
            truncate_str(&cd.artist, 18),
            Style::default().fg(COLOR_TEXT_PRIMARY),
        )));

        if let Some(provider) = &cd.metadata_provider {
            info_lines.push(Line::from(vec![
                Span::styled("✔ ", Style::default().fg(COLOR_SUCCESS).bold()),
                Span::styled(provider.clone(), Style::default().fg(COLOR_SUCCESS)),
            ]));
        } else {
            info_lines.push(Line::from(Span::styled(
                "---",
                Style::default().fg(COLOR_TEXT_DIM),
            )));
        }

        let total_tracks = cd.tracks.len();
        let total_secs: u32 = if let Some(ref toc) = cd.toc {
            let start = toc.get_track_offset(1).unwrap_or(150);
            (toc.leadout_offset.saturating_sub(start)) / 75
        } else {
            (total_tracks as u32) * 210
        };
        info_lines.push(Line::from(Span::styled(
            format!(
                "{total_tracks} {} • {:02}:{:02}",
                app.lang.tracks(),
                total_secs / 60,
                total_secs % 60
            ),
            Style::default().fg(COLOR_TEXT_MUTED),
        )));
    } else {
        info_lines.push(Line::from(Span::styled(
            app.lang.no_cd_detected(),
            Style::default().fg(COLOR_TEXT_MUTED).bold(),
        )));
        info_lines.push(Line::from(Span::styled(
            app.lang.insert_cd_prompt(),
            Style::default().fg(COLOR_TEXT_DIM),
        )));
    }

    let info_p = Paragraph::new(info_lines);
    f.render_widget(info_p, content_split[1]);
}

fn render_format_block(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let block = Block::default()
        .title(format!(" 🎵 {} [f/b] ", app.lang.format_and_quality()))
        .title_style(Style::default().fg(COLOR_TEXT_MUTED).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Format pills row
    let mut format_spans = vec![Span::styled(
        "Format:  ",
        Style::default().fg(COLOR_TEXT_MUTED),
    )];
    for (idx, (name, _)) in FORMATS.iter().enumerate() {
        if idx == app.format_index {
            format_spans.push(Span::styled(
                format!(" [{name}] "),
                Style::default()
                    .fg(COLOR_ACCENT)
                    .bg(Color::Rgb(20, 36, 48))
                    .bold(),
            ));
        } else {
            format_spans.push(Span::styled(
                format!("  {name}  "),
                Style::default().fg(COLOR_TEXT_DIM),
            ));
        }
    }

    // Bitrate pills row
    let is_lossy = matches!(app.encoder_key(), "mp3" | "opus" | "ogg");
    let mut bitrate_spans = vec![Span::styled(
        "Bitrate: ",
        Style::default().fg(COLOR_TEXT_MUTED),
    )];
    if is_lossy {
        for (idx, (label, _)) in BITRATES.iter().enumerate() {
            if idx == app.bitrate_index {
                bitrate_spans.push(Span::styled(
                    format!(" [{label}] "),
                    Style::default()
                        .fg(COLOR_ACCENT)
                        .bg(Color::Rgb(20, 36, 48))
                        .bold(),
                ));
            } else {
                bitrate_spans.push(Span::styled(
                    format!("  {label}  "),
                    Style::default().fg(COLOR_TEXT_DIM),
                ));
            }
        }
    } else {
        bitrate_spans.push(Span::styled(
            "Lossless (1411 kbps)",
            Style::default().fg(COLOR_SUCCESS),
        ));
    }

    let p = Paragraph::new(vec![Line::from(format_spans), Line::from(bitrate_spans)]);
    f.render_widget(p, inner);
}

fn render_output_dir_block(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let block = Block::default()
        .title(format!(" 📁 {} [o] ", app.lang.output_dir()))
        .title_style(Style::default().fg(COLOR_TEXT_MUTED).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let display_path = truncate_str(&app.output_dir, (inner.width as usize).saturating_sub(4));
    let line = Line::from(vec![
        Span::styled("📂 ", Style::default().fg(COLOR_ACCENT)),
        Span::styled(display_path, Style::default().fg(COLOR_TEXT_PRIMARY)),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn render_rip_control_block(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let is_focused = app.focus_sidebar;
    let block_border_color = if is_focused {
        COLOR_BORDER_FOCUS
    } else {
        COLOR_BORDER
    };

    let block = Block::default()
        .title(format!(" ⚙️ {} ", app.lang.tab_rip()))
        .title_style(Style::default().fg(COLOR_TEXT_MUTED).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(block_border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();

    if app.is_ripping {
        // Active Ripping state with progress gauges
        lines.push(Line::from(Span::styled(
            format!("⏹ [c] {}", app.lang.cancel_ripping()),
            Style::default()
                .fg(COLOR_DANGER)
                .bg(Color::Rgb(45, 20, 25))
                .bold(),
        )));

        let elapsed_secs = app
            .rip_start_time
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                format!(
                    "{} ({:02}:{:02})",
                    app.progress_label,
                    elapsed_secs / 60,
                    elapsed_secs % 60
                ),
                Style::default().fg(COLOR_ACCENT).bold(),
            ),
        ]));

        let p = Paragraph::new(lines);
        f.render_widget(p, Rect::new(inner.x, inner.y, inner.width, 2));

        // Overall progress gauge
        let gauge_pct = (app.overall_progress_fraction * 100.0).round() as u16;
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(app.lang.total_progress())
                    .borders(Borders::NONE),
            )
            .gauge_style(Style::default().fg(COLOR_ACCENT).bg(COLOR_CARD_BG))
            .percent(gauge_pct.min(100));

        f.render_widget(gauge, Rect::new(inner.x, inner.y + 2, inner.width, 2));

        // Track progress gauge
        let track_gauge_pct = (app.track_progress_fraction * 100.0).round() as u16;
        let track_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(app.lang.current_track())
                    .borders(Borders::NONE),
            )
            .gauge_style(Style::default().fg(COLOR_SUCCESS).bg(COLOR_CARD_BG))
            .percent(track_gauge_pct.min(100));

        f.render_widget(track_gauge, Rect::new(inner.x, inner.y + 4, inner.width, 2));
    } else {
        // Idle state: big primary action button
        let can_rip = app.cd_info.is_some() && app.selected_tracks.iter().any(|&s| s);
        let (btn_text, btn_style) = if can_rip {
            (
                format!("  ▶ [r] {}  ", app.lang.start_ripping()),
                Style::default()
                    .fg(Color::Rgb(10, 22, 28))
                    .bg(COLOR_ACCENT)
                    .bold(),
            )
        } else {
            (
                format!("  [r] {}  ", app.lang.no_cd_detected()),
                Style::default().fg(COLOR_TEXT_DIM).bg(COLOR_CARD_BG),
            )
        };

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(btn_text, btn_style)));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Selected: ", Style::default().fg(COLOR_TEXT_MUTED)),
            Span::styled(
                format!(
                    "{} / {} {}",
                    app.selected_tracks.iter().filter(|&&s| s).count(),
                    app.selected_tracks.len(),
                    app.lang.tracks(),
                ),
                Style::default().fg(COLOR_TEXT_PRIMARY).bold(),
            ),
        ]));

        let p = Paragraph::new(lines).alignment(Alignment::Center);
        f.render_widget(p, inner);
    }
}

fn render_tracklist(f: &mut Frame, app: &mut OxiDiscTuiApp, area: Rect) {
    let is_focused = !app.focus_sidebar;
    let border_color = if is_focused {
        COLOR_BORDER_FOCUS
    } else {
        COLOR_BORDER
    };

    let selected_count = app.selected_tracks.iter().filter(|&&s| s).count();
    let total_tracks = app.selected_tracks.len();

    let title = format!(
        " 📋 {} ({selected_count}/{total_tracks}) • [Space] | [a] {} | [n] {} ",
        app.lang.tracks(),
        app.lang.select_all(),
        app.lang.select_none()
    );
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(COLOR_TEXT_MUTED).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    if let Some(cd) = &app.cd_info {
        let toc = cd.toc.clone();
        let rows: Vec<Row> = cd
            .tracks
            .iter()
            .enumerate()
            .map(|(idx, title)| {
                let track_num = idx + 1;
                let is_selected = app.selected_tracks.get(idx).copied().unwrap_or(true);
                let is_completed = app.completed_tracks.get(idx).copied().unwrap_or(false);
                let is_active = app.is_ripping && app.current_ripping_track == Some(track_num);

                let check_span = if is_selected {
                    Span::styled("[✔]", Style::default().fg(COLOR_ACCENT).bold())
                } else {
                    Span::styled("[ ]", Style::default().fg(COLOR_TEXT_DIM))
                };

                let num_span = Span::styled(
                    format!("{:02}", track_num),
                    if is_active {
                        Style::default().fg(COLOR_ACCENT).bold()
                    } else {
                        Style::default().fg(COLOR_TEXT_MUTED)
                    },
                );

                let title_style = if is_active {
                    Style::default().fg(Color::White).bold()
                } else if is_selected {
                    Style::default().fg(COLOR_TEXT_PRIMARY)
                } else {
                    Style::default().fg(COLOR_TEXT_DIM)
                };
                let title_span = Span::styled(title.clone(), title_style);

                let duration_str = if let Some(ref t) = toc {
                    let start = t.get_track_offset(track_num as u8).unwrap_or(150);
                    let end = if (track_num as u8) < t.last_track {
                        t.get_track_offset((track_num + 1) as u8)
                            .unwrap_or(start + 1500)
                    } else {
                        t.leadout_offset
                    };
                    let secs = (end.saturating_sub(start)) / 75;
                    format!("{:02}:{:02}", secs / 60, secs % 60)
                } else {
                    "--:--".to_string()
                };
                let dur_span = Span::styled(duration_str, Style::default().fg(COLOR_TEXT_MUTED));

                let status_span = if is_completed {
                    Span::styled(
                        format!("✔ {}", app.lang.status_done()),
                        Style::default().fg(COLOR_SUCCESS).bold(),
                    )
                } else if is_active {
                    let pct = (app.track_progress_fraction * 100.0).round() as u32;
                    Span::styled(
                        format!("⏳ {} {pct}%", app.lang.status_ripping()),
                        Style::default().fg(COLOR_ACCENT).bold(),
                    )
                } else if is_selected {
                    Span::styled(
                        app.lang.status_ready(),
                        Style::default().fg(COLOR_TEXT_MUTED),
                    )
                } else {
                    Span::styled(
                        app.lang.status_skipped(),
                        Style::default().fg(COLOR_TEXT_DIM),
                    )
                };

                let row_bg = if idx % 2 == 0 {
                    COLOR_CARD_BG
                } else {
                    COLOR_CARD_BG_ALT
                };

                Row::new(vec![
                    Cell::from(check_span),
                    Cell::from(num_span),
                    Cell::from(title_span),
                    Cell::from(dur_span),
                    Cell::from(status_span),
                ])
                .style(Style::default().bg(row_bg))
                .height(1)
            })
            .collect();

        let header_titles = [
            "Sel",
            "#",
            app.lang.col_title(),
            app.lang.col_duration(),
            app.lang.col_status(),
        ];
        let header_cells = header_titles
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(COLOR_TEXT_MUTED).bold()));
        let header = Row::new(header_cells)
            .style(Style::default().bg(Color::Rgb(20, 26, 36)))
            .height(1);

        let table = Table::new(
            rows,
            [
                Constraint::Length(5),  // Sel
                Constraint::Length(4),  // #
                Constraint::Min(20),    // Title
                Constraint::Length(8),  // Duration
                Constraint::Length(14), // Status
            ],
        )
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 45, 65))
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always);

        f.render_stateful_widget(table, area, &mut app.table_state);
    } else {
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("💿 {}", app.lang.no_cd_detected()),
                Style::default().fg(COLOR_TEXT_MUTED).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                app.lang.insert_cd_prompt(),
                Style::default().fg(COLOR_TEXT_DIM),
            )),
        ])
        .alignment(Alignment::Center)
        .block(block);

        f.render_widget(empty_msg, area);
    }
}

fn render_settings_view(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let block = Block::default()
        .title(format!(
            " ⚙️ {} [↑/↓ | ←/→/Enter | [s] {}] ",
            app.lang.tab_settings(),
            app.lang.settings_save()
        ))
        .title_style(Style::default().fg(COLOR_ACCENT).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lang_str = match app.language_index {
        1 => "English",
        2 => "Deutsch",
        3 => "Français",
        4 => "Italiano",
        5 => "Español",
        6 => "Português",
        7 => "日本語",
        8 => "Русский",
        9 => "Polski",
        10 => "中文",
        _ => "Auto (System)",
    };

    let items = [
        (app.lang.settings_language(), lang_str.to_string()),
        (
            app.lang.settings_metadata_source(),
            app.metadata_source_key().to_string(),
        ),
        (app.lang.format_and_quality(), app.encoder_key().to_string()),
        ("Bitrate", app.bitrate_key().to_string()),
        (
            app.lang.settings_auto_create_dir(),
            if app.auto_create_output_dir {
                "[x] On".to_string()
            } else {
                "[ ] Off".to_string()
            },
        ),
        (app.lang.output_dir(), app.output_dir.clone()),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (idx, (label, val)) in items.iter().enumerate() {
        let is_selected = app.settings_cursor == idx;
        let cursor_str = if is_selected { "▶ " } else { "  " };
        let label_style = if is_selected {
            Style::default().fg(COLOR_ACCENT).bold()
        } else {
            Style::default().fg(COLOR_TEXT_PRIMARY)
        };
        let val_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(30, 45, 65))
                .bold()
        } else {
            Style::default().fg(COLOR_TEXT_MUTED)
        };

        lines.push(Line::from(vec![
            Span::styled(cursor_str, Style::default().fg(COLOR_ACCENT).bold()),
            Span::styled(format!("{:<38} : ", label), label_style),
            Span::styled(format!(" {val} "), val_style),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("[s] {}...", app.lang.settings_save()),
        Style::default().fg(COLOR_ACCENT_DIM).italic(),
    )));

    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

fn render_help_view(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let block = Block::default()
        .title(app.lang.tui_help_title())
        .title_style(Style::default().fg(COLOR_ACCENT).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let help_text = match app.lang {
        crate::i18n::Language::De => vec![
            Line::from(Span::styled(
                "Tastaturnavigation & Shortcuts",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Rip-Ansicht (Hauptansicht)"),
            Line::from("  [2]           Einstellungen"),
            Line::from("  [3]           Hilfe"),
            Line::from("  [Tab]         Fokus umschalten (Sidebar <-> Trackliste)"),
            Line::from(""),
            Line::from("  [Space]       Ausgewählten Track an- oder abwählen"),
            Line::from("  [a]           Alle Tracks auswählen"),
            Line::from("  [n]           Alle Tracks abwählen"),
            Line::from("  [↑ / k]       Vorheriger Track"),
            Line::from("  [↓ / j]       Nächster Track"),
            Line::from(""),
            Line::from("  [r]           CD-Rippen starten (oder abbrechen)"),
            Line::from("  [c]           Rippen abbrechen"),
            Line::from("  [f / F]       Audio-Format durchschalten (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Bitrate durchschalten (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Ausgabe-Verzeichnis bearbeiten"),
            Line::from("  [e]           ⏏ CD auswerfen"),
            Line::from("  [l]           📥 CD-Laufwerk einziehen / schließen"),
            Line::from("  [u]           🔄 Neu einlesen / Disc erkennen"),
            Line::from("  [q / Ctrl+C]  Beenden"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Tipp: Mit [Esc] oder [Enter] gelangst du zurück zur Rip-Ansicht.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Fr => vec![
            Line::from(Span::styled(
                "Navigation au clavier & Raccourcis",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Vue extraction (Principale)"),
            Line::from("  [2]           Paramètres"),
            Line::from("  [3]           Aide"),
            Line::from("  [Tab]         Changer le focus (Volet <-> Pistes)"),
            Line::from(""),
            Line::from("  [Space]       Sélectionner / Désélectionner la piste"),
            Line::from("  [a]           Tout sélectionner"),
            Line::from("  [n]           Tout désélectionner"),
            Line::from("  [↑ / k]       Piste précédente"),
            Line::from("  [↓ / j]       Piste suivante"),
            Line::from(""),
            Line::from("  [r]           Démarrer l'extraction (ou annuler)"),
            Line::from("  [c]           Annuler l'extraction"),
            Line::from("  [f / F]       Changer le format audio (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Changer le débit binaire (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Modifier le dossier de sortie"),
            Line::from("  [e]           ⏏ Éjecter le CD"),
            Line::from("  [l]           📥 Charger / Fermer le lecteur"),
            Line::from("  [u]           🔄 Actualiser / Détecter le disque"),
            Line::from("  [q / Ctrl+C]  Quitter"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Astuce : Appuyez sur [Esc] ou [Enter] pour revenir à la vue d'extraction.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Es => vec![
            Line::from(Span::styled(
                "Navegación por teclado y Atajos",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Vista de extracción (Principal)"),
            Line::from("  [2]           Configuración"),
            Line::from("  [3]           Ayuda"),
            Line::from("  [Tab]         Cambiar enfoque (Lateral <-> Pistas)"),
            Line::from(""),
            Line::from("  [Space]       Activar / Desactivar pista"),
            Line::from("  [a]           Seleccionar todas"),
            Line::from("  [n]           Deseleccionar todas"),
            Line::from("  [↑ / k]       Pista anterior"),
            Line::from("  [↓ / j]       Pista siguiente"),
            Line::from(""),
            Line::from("  [r]           Iniciar extracción (o cancelar)"),
            Line::from("  [c]           Cancelar extracción"),
            Line::from("  [f / F]       Cambiar formato de audio (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Cambiar bitrate (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Editar carpeta de salida"),
            Line::from("  [e]           ⏏ Expulsar CD"),
            Line::from("  [l]           📥 Cargar / Cerrar bandeja"),
            Line::from("  [u]           🔄 Actualizar / Detectar disco"),
            Line::from("  [q / Ctrl+C]  Salir"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Consejo: Presione [Esc] o [Enter] para volver a la vista de extracción.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::It => vec![
            Line::from(Span::styled(
                "Navigazione da tastiera e Scorciatoie",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Vista estrazione (Principale)"),
            Line::from("  [2]           Impostazioni"),
            Line::from("  [3]           Aiuto"),
            Line::from("  [Tab]         Cambia selezione (Pannello <-> Tracce)"),
            Line::from(""),
            Line::from("  [Space]       Seleziona / Deseleziona traccia"),
            Line::from("  [a]           Seleziona tutte le tracce"),
            Line::from("  [n]           Deseleziona tutte le tracce"),
            Line::from("  [↑ / k]       Traccia precedente"),
            Line::from("  [↓ / j]       Traccia successiva"),
            Line::from(""),
            Line::from("  [r]           Avvia estrazione CD (o annulla)"),
            Line::from("  [c]           Annulla estrazione"),
            Line::from("  [f / F]       Cambia formato audio (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Cambia bitrate (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Modifica cartella di destinazione"),
            Line::from("  [e]           ⏏ Espelli CD"),
            Line::from("  [l]           📥 Carica / Chiudi carrello CD"),
            Line::from("  [u]           🔄 Aggiorna / Rileva disco"),
            Line::from("  [q / Ctrl+C]  Esci"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Suggerimento: Premi [Esc] o [Invio] per tornare alla vista estrazione.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Pt => vec![
            Line::from(Span::styled(
                "Navegação por Teclado e Atalhos",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Vista de Extração (Principal)"),
            Line::from("  [2]           Configurações"),
            Line::from("  [3]           Ajuda"),
            Line::from("  [Tab]         Alternar foco (Painel <-> Faixas)"),
            Line::from(""),
            Line::from("  [Space]       Marcar / Desmarcar faixa"),
            Line::from("  [a]           Selecionar todas as faixas"),
            Line::from("  [n]           Desmarcar todas as faixas"),
            Line::from("  [↑ / k]       Faixa anterior"),
            Line::from("  [↓ / j]       Faixa seguinte"),
            Line::from(""),
            Line::from("  [r]           Iniciar extração do CD (ou cancelar)"),
            Line::from("  [c]           Cancelar extração"),
            Line::from("  [f / F]       Alternar formato de áudio (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Alternar taxa de bits (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Editar pasta de saída"),
            Line::from("  [e]           ⏏ Ejetar CD"),
            Line::from("  [l]           📥 Carregar / Fechar bandeja do CD"),
            Line::from("  [u]           🔄 Atualizar / Detectar disco"),
            Line::from("  [q / Ctrl+C]  Sair"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Dica: Pressione [Esc] ou [Enter] para voltar à vista de extração.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Ja => vec![
            Line::from(Span::styled(
                "キーボード操作とショートカット",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           リップ画面（メイン）"),
            Line::from("  [2]           設定"),
            Line::from("  [3]           ヘルプ"),
            Line::from("  [Tab]         フォーカス切り替え（サイドバー <-> トラック一覧）"),
            Line::from(""),
            Line::from("  [Space]       トラックの選択 / 解除"),
            Line::from("  [a]           全てのトラックを選択"),
            Line::from("  [n]           全てのトラックの選択を解除"),
            Line::from("  [↑ / k]       前のトラック"),
            Line::from("  [↓ / j]       次のトラック"),
            Line::from(""),
            Line::from("  [r]           CDリッピング開始（または中止）"),
            Line::from("  [c]           リッピング中止"),
            Line::from("  [f / F]       音声フォーマットの切り替え（FLAC, Opus, MP3, WAV, Ogg）"),
            Line::from("  [b / B]       ビットレートの切り替え（128, 192, 256, 320 kbps）"),
            Line::from("  [o]           出力先フォルダの変更"),
            Line::from("  [e]           ⏏ CDを取り出す"),
            Line::from("  [l]           📥 CDトレイを閉じる"),
            Line::from("  [u]           🔄 ディスクの再読込 / 検出"),
            Line::from("  [q / Ctrl+C]  終了"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 ヒント: [Esc] または [Enter] キーでリップ画面に戻ります。",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Ru => vec![
            Line::from(Span::styled(
                "Управление с клавиатуры и горячие клавиши",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Риппинг (Главный экран)"),
            Line::from("  [2]           Настройки"),
            Line::from("  [3]           Справка"),
            Line::from("  [Tab]         Переключить фокус (Панель <-> Треки)"),
            Line::from(""),
            Line::from("  [Space]       Выбрать / снять выбор трека"),
            Line::from("  [a]           Выбрать все треки"),
            Line::from("  [n]           Снять выбор со всех треков"),
            Line::from("  [↑ / k]       Предыдущий трек"),
            Line::from("  [↓ / j]       Следующий трек"),
            Line::from(""),
            Line::from("  [r]           Начать риппинг CD (или отменить)"),
            Line::from("  [c]           Отменить риппинг"),
            Line::from("  [f / F]       Переключить аудиоформат (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Переключить битрейт (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Изменить папку сохранения"),
            Line::from("  [e]           ⏏ Извлечь CD"),
            Line::from("  [l]           📥 Загрузить / закрыть лоток CD"),
            Line::from("  [u]           🔄 Обновить / считать диск"),
            Line::from("  [q / Ctrl+C]  Выход"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Подсказка: Нажмите [Esc] или [Enter], чтобы вернуться на экран риппинга.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Pl => vec![
            Line::from(Span::styled(
                "Nawigacja za pomocą klawiatury i skróty",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Widok zgrywania (Główny)"),
            Line::from("  [2]           Ustawienia"),
            Line::from("  [3]           Pomoc"),
            Line::from("  [Tab]         Przełącz fokus (Panel boczny <-> Lista ścieżek)"),
            Line::from(""),
            Line::from("  [Space]       Włącz / Wyłącz zaznaczenie ścieżki"),
            Line::from("  [a]           Zaznacz wszystkie"),
            Line::from("  [n]           Odznacz wszystkie"),
            Line::from("  [↑ / k]       Poprzednia ścieżka"),
            Line::from("  [↓ / j]       Następna ścieżka"),
            Line::from(""),
            Line::from("  [r]           Rozpocznij zgrywanie CD (lub anuluj)"),
            Line::from("  [c]           Anuluj zgrywanie"),
            Line::from("  [f / F]       Przełącz format audio (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Przełącz bitrate (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Edytuj katalog wyjściowy"),
            Line::from("  [e]           ⏏ Wysuń płytę CD"),
            Line::from("  [l]           📥 Wsuń / Zamknij tackę CD"),
            Line::from("  [u]           🔄 Odśwież / Wykryj płytę"),
            Line::from("  [q / Ctrl+C]  Wyjdź"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Wskazówka: Naciśnij [Esc] lub [Enter], aby powrócić do widoku zgrywania.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        crate::i18n::Language::Zh => vec![
            Line::from(Span::styled(
                "键盘导航与快捷键指南",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           抓取视图（主界面）"),
            Line::from("  [2]           设置"),
            Line::from("  [3]           帮助"),
            Line::from("  [Tab]         切换焦点（侧边栏 <-> 曲目列表）"),
            Line::from(""),
            Line::from("  [Space]       选择 / 取消选中当前曲目"),
            Line::from("  [a]           全选所有曲目"),
            Line::from("  [n]           取消全选"),
            Line::from("  [↑ / k]       上一曲目"),
            Line::from("  [↓ / j]       下一曲目"),
            Line::from(""),
            Line::from("  [r]           开始抓取 CD（或取消）"),
            Line::from("  [c]           取消抓取"),
            Line::from("  [f / F]       切换音频格式 (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       切换比特率 (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           编辑输出目录"),
            Line::from("  [e]           ⏏ 弹出 CD"),
            Line::from("  [l]           📥 加载 / 关闭 CD 托盘"),
            Line::from("  [u]           🔄 刷新 / 检测光盘"),
            Line::from("  [q / Ctrl+C]  退出"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 提示: 按 [Esc] 或 [Enter] 键即可返回抓取视图。",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
        _ => vec![
            Line::from(Span::styled(
                "Keyboard Navigation & Shortcuts",
                Style::default().fg(COLOR_ACCENT).bold(),
            )),
            Line::from(""),
            Line::from("  [1]           Rip View (Main)"),
            Line::from("  [2]           Settings"),
            Line::from("  [3]           Help"),
            Line::from("  [Tab]         Toggle focus (Sidebar <-> Tracklist)"),
            Line::from(""),
            Line::from("  [Space]       Toggle track selection"),
            Line::from("  [a]           Select all tracks"),
            Line::from("  [n]           Deselect all tracks"),
            Line::from("  [↑ / k]       Previous track"),
            Line::from("  [↓ / j]       Next track"),
            Line::from(""),
            Line::from("  [r]           Start ripping CD (or cancel)"),
            Line::from("  [c]           Cancel ripping"),
            Line::from("  [f / F]       Cycle audio format (FLAC, Opus, MP3, WAV, Ogg)"),
            Line::from("  [b / B]       Cycle bitrate (128, 192, 256, 320 kbps)"),
            Line::from("  [o]           Edit output directory"),
            Line::from("  [e]           ⏏ Eject CD"),
            Line::from("  [l]           📥 Load / Close CD tray"),
            Line::from("  [u]           🔄 Refresh / Detect disc"),
            Line::from("  [q / Ctrl+C]  Quit"),
            Line::from(""),
            Line::from(Span::styled(
                "💡 Tip: Press [Esc] or [Enter] to return to the Rip View.",
                Style::default().fg(COLOR_SUCCESS),
            )),
        ],
    };

    let p = Paragraph::new(help_text);
    f.render_widget(p, inner);
}

fn render_footer(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Status Line
    let status_line = Line::from(vec![
        Span::styled(
            " STATUS: ",
            Style::default()
                .fg(Color::Rgb(10, 22, 28))
                .bg(COLOR_ACCENT)
                .bold(),
        ),
        Span::styled(
            format!(" {} ", app.status),
            Style::default().fg(COLOR_TEXT_PRIMARY),
        ),
    ]);
    f.render_widget(Paragraph::new(status_line), chunks[0]);

    // Shortcuts Hint Line
    let hints_line = Line::from(Span::styled(
        format!(" {}", app.lang.tui_hints()),
        Style::default().fg(COLOR_TEXT_MUTED),
    ));
    f.render_widget(Paragraph::new(hints_line), chunks[1]);
}

fn render_output_dir_modal(f: &mut Frame, app: &OxiDiscTuiApp, area: Rect) {
    let popup_area = centered_rect(60, 25, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" 📁 Ausgabe-Verzeichnis ändern ")
        .title_style(Style::default().fg(COLOR_ACCENT).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_ACCENT));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let modal_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner);

    let prompt = Paragraph::new(
        "Pfad eingeben und [Enter] zum Bestätigen oder [Esc] zum Abbrechen drücken:",
    );
    f.render_widget(prompt, modal_chunks[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_ACCENT));
    let input_text = Paragraph::new(format!("{}_", app.temp_output_dir))
        .style(Style::default().fg(Color::White).bold())
        .block(input_block);
    f.render_widget(input_text, modal_chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    }
}

fn render_vinyl_ascii(has_cd: bool, is_ripping: bool) -> Vec<Line<'static>> {
    if has_cd {
        let disc_color = if is_ripping {
            COLOR_ACCENT
        } else {
            Color::Rgb(160, 185, 210)
        };
        let center_color = if is_ripping {
            COLOR_SUCCESS
        } else {
            COLOR_ACCENT_DIM
        };
        vec![
            Line::from(vec![Span::styled(
                "   ╭─────────────╮",
                Style::default().fg(Color::Rgb(60, 75, 95)),
            )]),
            Line::from(vec![
                Span::styled("   │ ", Style::default().fg(Color::Rgb(60, 75, 95))),
                Span::styled(" .─────────. ", Style::default().fg(disc_color).bold()),
                Span::styled(" │", Style::default().fg(Color::Rgb(60, 75, 95))),
            ]),
            Line::from(vec![
                Span::styled("   │ ", Style::default().fg(Color::Rgb(60, 75, 95))),
                Span::styled("/   ", Style::default().fg(disc_color)),
                Span::styled(" (o) ", Style::default().fg(center_color).bold()),
                Span::styled("   \\", Style::default().fg(disc_color)),
                Span::styled(" │", Style::default().fg(Color::Rgb(60, 75, 95))),
            ]),
            Line::from(vec![
                Span::styled("   │ ", Style::default().fg(Color::Rgb(60, 75, 95))),
                Span::styled("\\           /", Style::default().fg(disc_color)),
                Span::styled(" │", Style::default().fg(Color::Rgb(60, 75, 95))),
            ]),
            Line::from(vec![
                Span::styled("   │ ", Style::default().fg(Color::Rgb(60, 75, 95))),
                Span::styled(" '─────────' ", Style::default().fg(disc_color).bold()),
                Span::styled(" │", Style::default().fg(Color::Rgb(60, 75, 95))),
            ]),
            Line::from(vec![Span::styled(
                "   ╰─────────────╯",
                Style::default().fg(Color::Rgb(60, 75, 95)),
            )]),
        ]
    } else {
        vec![
            Line::from(vec![Span::styled(
                "   ╭─────────────╮",
                Style::default().fg(Color::Rgb(50, 60, 75)),
            )]),
            Line::from(vec![Span::styled(
                "   │  .───────.  │",
                Style::default().fg(Color::Rgb(70, 85, 105)),
            )]),
            Line::from(vec![Span::styled(
                "   │ /   ---   \\ │",
                Style::default().fg(Color::Rgb(70, 85, 105)),
            )]),
            Line::from(vec![Span::styled(
                "   │ \\         / │",
                Style::default().fg(Color::Rgb(70, 85, 105)),
            )]),
            Line::from(vec![Span::styled(
                "   │  '───────'  │",
                Style::default().fg(Color::Rgb(70, 85, 105)),
            )]),
            Line::from(vec![Span::styled(
                "   ╰─────────────╯",
                Style::default().fg(Color::Rgb(50, 60, 75)),
            )]),
        ]
    }
}
