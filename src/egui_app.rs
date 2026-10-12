use crate::cd_reader::{CdInfo, CdReader};
use crate::config::Config;
use crate::i18n::Language;
use crate::ripper::{RipMessage, Ripper};
use egui::{Color32, CornerRadius, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::path::PathBuf;
#[cfg(not(windows))]
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppTab {
    Rip,
    Settings,
}

pub struct OxiDiscEguiApp {
    active_tab: AppTab,
    cd_info: Option<CdInfo>,
    language_index: usize,
    pub lang: Language,
    format_index: usize,
    bitrate_index: usize,
    ui_backend_index: usize,
    metadata_source_index: usize,
    album_art_size_index: usize,
    album_art_download_index: usize,
    output_dir: String,
    auto_create_output_dir: bool,
    show_create_dir_modal: bool,
    pending_create_dir_path: Option<PathBuf>,
    dont_ask_create_dir_checkbox: bool,
    show_missing_encoder_modal: bool,
    missing_encoder_name: String,
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
    album_cover_texture: Option<egui::TextureHandle>,
    album_cover_source_url: Option<String>,
    album_cover_error: Option<String>,
    next_auto_detect_at: Instant,
    status: String,
}

impl Default for OxiDiscEguiApp {
    fn default() -> Self {
        let cfg = Config::load();
        let (language_index, lang) = match cfg.language.to_lowercase().as_str() {
            "en" => (1, Language::En),
            "de" => (2, Language::De),
            "fr" => (3, Language::Fr),
            "it" => (4, Language::It),
            "es" => (5, Language::Es),
            "pt" => (6, Language::Pt),
            "ja" => (7, Language::Ja),
            "ru" => (8, Language::Ru),
            "pl" => (9, Language::Pl),
            "zh" => (10, Language::Zh),
            _ => (0, Language::resolve(&cfg.language)),
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
        let album_art_size_index = match cfg.album_art_size_preference.as_str() {
            "small" => 1,
            "large" => 2,
            "original" => 3,
            _ => 0,
        };
        let album_art_download_index = match cfg.album_art_download_behavior.as_str() {
            "save-with-rip" => 1,
            _ => 0,
        };
        let ui_backend_index = match cfg.ui_backend.as_str() {
            "tui" if cfg!(feature = "tui") => 1,
            _ => 0,
        };
        let output_dir = std::env::var("OXIDISC_OUTPUT_DIR")
            .ok()
            .or_else(|| dirs::audio_dir().map(|p| p.to_string_lossy().to_string()))
            .or_else(|| {
                dirs::home_dir().map(|home| home.join("Music").to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "~/Music".to_string());

        Self {
            active_tab: AppTab::Rip,
            cd_info: None,
            language_index,
            lang,
            format_index,
            bitrate_index,
            ui_backend_index,
            metadata_source_index,
            album_art_size_index,
            album_art_download_index,
            output_dir,
            auto_create_output_dir: cfg.auto_create_output_dir,
            show_create_dir_modal: false,
            pending_create_dir_path: None,
            dont_ask_create_dir_checkbox: false,
            show_missing_encoder_modal: false,
            missing_encoder_name: String::new(),
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
            album_cover_texture: None,
            album_cover_source_url: None,
            album_cover_error: None,
            next_auto_detect_at: Instant::now(),
            status: String::new(),
        }
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 780.0])
            .with_min_inner_size([880.0, 600.0])
            .with_title("OxiDisc"),
        ..Default::default()
    };
    eframe::run_native(
        "OxiDisc",
        options,
        Box::new(|cc| {
            setup_custom_fonts(&cc.egui_ctx);
            setup_custom_theme(&cc.egui_ctx);
            Ok(Box::<OxiDiscEguiApp>::default())
        }),
    )
}

fn find_system_cjk_font_paths() -> Vec<PathBuf> {
    let mut found = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let fonts_dir = PathBuf::from(windir).join("Fonts");
        let win_candidates = [
            "msyh.ttc",
            "msyh.ttf",
            "msyhl.ttc",
            "yugothr.ttc",
            "meiryo.ttc",
            "msgothic.ttc",
            "simsun.ttc",
            "mingliu.ttc",
            "malgun.ttf",
        ];
        for file_name in win_candidates {
            let p = fonts_dir.join(file_name);
            if p.exists() && !found.contains(&p) {
                found.push(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mac_candidates = [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/Supplemental/Songti.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ];
        for p in mac_candidates {
            let pb = PathBuf::from(p);
            if pb.exists() && !found.contains(&pb) {
                found.push(pb);
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // 1. Query fontconfig via fc-list for Japanese and Chinese fonts if available
        for lang in [":lang=ja", ":lang=zh"] {
            if let Ok(output) = std::process::Command::new("fc-list")
                .args([lang, "file"])
                .output()
            {
                if output.status.success() {
                    let out_str = String::from_utf8_lossy(&output.stdout);
                    for line in out_str.lines() {
                        let path_str = line.trim().trim_end_matches(':');
                        if !path_str.is_empty() {
                            let pb = PathBuf::from(path_str);
                            if pb.exists() && !found.contains(&pb) {
                                found.push(pb);
                            }
                        }
                    }
                }
            }
        }

        // 2. Common static Linux font paths as fallback
        let linux_candidates = [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
            "/usr/share/fonts/opentype/ipafont-mincho/ipam.ttf",
            "/usr/share/fonts/truetype/takao-gothic/TakaoPGothic.ttf",
            "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
            "/usr/share/fonts/cjkuni-uming/uming.ttc",
            "/usr/share/fonts/cjkuni-ukai/ukai.ttc",
            "/usr/share/fonts/truetype/arphic/ukai.ttc",
            "/usr/share/fonts/truetype/vlgothic/VL-PGothic-Regular.ttf",
        ];
        for p in linux_candidates {
            let pb = PathBuf::from(p);
            if pb.exists() && !found.contains(&pb) {
                found.push(pb);
            }
        }
    }

    found
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let cjk_paths = find_system_cjk_font_paths();

    let mut loaded_count = 0;
    for path in cjk_paths {
        if let Ok(font_bytes) = std::fs::read(&path) {
            let font_name = format!("cjk_fallback_{}", loaded_count);
            fonts.font_data.insert(
                font_name.clone(),
                std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
            );

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push(font_name.clone());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(font_name);

            loaded_count += 1;
            // Up to 2 CJK fonts is sufficient to provide comprehensive glyph coverage
            if loaded_count >= 2 {
                break;
            }
        }
    }

    if loaded_count > 0 {
        ctx.set_fonts(fonts);
    }
}

fn setup_custom_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.dark_mode = true;
    visuals.override_text_color = Some(Color32::from_rgb(236, 242, 250));

    // Deep Obsidian / Dark Slate Backgrounds
    visuals.panel_fill = Color32::from_rgb(16, 20, 26);
    visuals.window_fill = Color32::from_rgb(16, 20, 26);
    visuals.faint_bg_color = Color32::from_rgb(22, 28, 38);
    visuals.extreme_bg_color = Color32::from_rgb(10, 13, 18);
    visuals.code_bg_color = Color32::from_rgb(24, 30, 42);

    // Widgets styling (smooth roundings, subtle border strokes)
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(22, 28, 38);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(38, 48, 64));
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(24, 32, 44);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(40, 52, 70));
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(32, 44, 60);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0, 220, 200));
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = Color32::from_rgb(0, 180, 165);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, Color32::from_rgb(0, 240, 220));
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    visuals.selection.bg_fill = Color32::from_rgb(0, 200, 180);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(0, 240, 220));

    ctx.set_visuals(visuals);
}

impl eframe::App for OxiDiscEguiApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.063, 0.078, 0.102, 1.0] // #10141a
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_ripper_messages();

        if self.is_ripping {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }

        // Top Navigation Bar
        self.render_top_header(ui);

        ui.add_space(8.0);

        // Main Content Area
        egui::Frame::NONE
            .fill(Color32::from_rgb(16, 20, 26))
            .inner_margin(egui::Margin::symmetric(20, 12))
            .show(ui, |ui| match self.active_tab {
                AppTab::Rip => self.render_modern_rip_view(ui),
                AppTab::Settings => self.render_settings_tab(ui),
            });

        // Modal Confirmation for missing destination directories
        self.render_create_dir_modal(ui.ctx());

        // Modal Warning for missing audio encoder CLI tools
        self.render_missing_encoder_modal(ui.ctx());
    }
}

impl OxiDiscEguiApp {
    const AUTO_DETECT_RETRY_INTERVAL: Duration = Duration::from_secs(2);

    pub fn update_language_from_index(&mut self) {
        self.lang = match self.language_index {
            1 => Language::En,
            2 => Language::De,
            3 => Language::Fr,
            4 => Language::It,
            5 => Language::Es,
            6 => Language::Pt,
            7 => Language::Ja,
            8 => Language::Ru,
            9 => Language::Pl,
            10 => Language::Zh,
            _ => Language::resolve("auto"),
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

    fn render_top_header(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(19, 24, 32))
            .stroke(Stroke::new(1.0, Color32::from_rgb(32, 40, 54)))
            .inner_margin(egui::Margin::symmetric(20, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // App Logo / Title
                    ui.label(RichText::new("💿").size(20.0));
                    ui.label(
                        RichText::new("OxiDisc")
                            .strong()
                            .size(17.0)
                            .color(Color32::from_rgb(240, 246, 255)),
                    );

                    ui.add_space(20.0);

                    // Tab Selector Pills
                    let is_rip_tab = self.active_tab == AppTab::Rip;
                    let rip_btn = egui::Button::new(
                        RichText::new(self.lang.tab_rip())
                            .size(13.0)
                            .color(if is_rip_tab {
                                Color32::from_rgb(10, 20, 25)
                            } else {
                                Color32::from_rgb(180, 195, 215)
                            })
                            .strong(),
                    )
                    .fill(if is_rip_tab {
                        Color32::from_rgb(0, 220, 200)
                    } else {
                        Color32::from_rgb(26, 33, 44)
                    })
                    .corner_radius(CornerRadius::same(14));

                    if ui.add(rip_btn).clicked() {
                        self.active_tab = AppTab::Rip;
                    }

                    let is_settings_tab = self.active_tab == AppTab::Settings;
                    let settings_btn = egui::Button::new(
                        RichText::new(self.lang.tab_settings())
                            .size(13.0)
                            .color(if is_settings_tab {
                                Color32::from_rgb(10, 20, 25)
                            } else {
                                Color32::from_rgb(180, 195, 215)
                            })
                            .strong(),
                    )
                    .fill(if is_settings_tab {
                        Color32::from_rgb(0, 220, 200)
                    } else {
                        Color32::from_rgb(26, 33, 44)
                    })
                    .corner_radius(CornerRadius::same(14));

                    if ui.add(settings_btn).clicked() {
                        self.active_tab = AppTab::Settings;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Device indicator badge
                        let dev = CdReader::active_device_path();
                        let drive_label = self.lang.drive();
                        let dev_text = if crate::mock_cd::is_mock_device(&dev) {
                            format!("{drive_label}: [Mock Virtual CD]")
                        } else {
                            format!("{drive_label}: [{dev}]")
                        };

                        egui::Frame::NONE
                            .fill(Color32::from_rgb(26, 34, 46))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(42, 54, 72)))
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(dev_text)
                                        .size(12.0)
                                        .color(Color32::from_rgb(140, 165, 195)),
                                );
                            });

                        // Eject disc button
                        let eject_text = format!("⏏ {}", self.lang.eject());
                        if ui
                            .add_enabled(
                                !self.is_ripping,
                                egui::Button::new(RichText::new(eject_text).size(12.0))
                                    .fill(Color32::from_rgb(26, 34, 46))
                                    .corner_radius(CornerRadius::same(12)),
                            )
                            .clicked()
                        {
                            self.eject_disc();
                        }

                        // Load / Pull in CD tray button
                        let load_text = format!("📥 {}", self.lang.load_tray());
                        if ui
                            .add_enabled(
                                !self.is_ripping,
                                egui::Button::new(RichText::new(load_text).size(12.0))
                                    .fill(Color32::from_rgb(26, 34, 46))
                                    .corner_radius(CornerRadius::same(12)),
                            )
                            .clicked()
                        {
                            self.load_disc();
                        }

                        // Refresh / Detect CD button
                        let refresh_text = format!("🔄 {}", self.lang.refresh());
                        if ui
                            .add_enabled(
                                !self.is_ripping,
                                egui::Button::new(RichText::new(refresh_text).size(12.0))
                                    .fill(Color32::from_rgb(26, 34, 46))
                                    .corner_radius(CornerRadius::same(12)),
                            )
                            .clicked()
                        {
                            self.detect_cd();
                        }
                    });
                });
            });
    }

    fn render_modern_rip_view(&mut self, ui: &mut egui::Ui) {
        self.maybe_auto_detect_cd(ui.ctx());

        let cover_url = self.current_album_cover_url();
        self.ensure_album_cover(ui.ctx(), cover_url.as_deref());

        let sidebar_width = 300.0;
        let available_size = ui.available_size();

        // 2-Column Split: Left Sidebar & Right Tracklist
        ui.horizontal(|ui| {
            // LEFT SIDEBAR
            ui.allocate_ui_with_layout(
                egui::vec2(sidebar_width, available_size.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_sidebar_panel(ui, sidebar_width);
                },
            );

            ui.add_space(16.0);

            // RIGHT MAIN AREA: Tracklist & Master Progress
            let right_width = (available_size.x - sidebar_width - 16.0).max(300.0);
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, available_size.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_tracklist_and_progress(ui, right_width);
                },
            );
        });
    }

    fn render_sidebar_panel(&mut self, ui: &mut egui::Ui, sidebar_width: f32) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(20, 26, 35))
            .stroke(Stroke::new(1.0, Color32::from_rgb(34, 44, 58)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(sidebar_width - 32.0);

                // 1. Album Cover Art Card
                let cover_size = Vec2::splat(sidebar_width - 32.0);
                self.render_cover_art_box(ui, cover_size);

                ui.add_space(14.0);

                // 2. Album Title & Artist
                if let Some(cd_info) = &self.cd_info {
                    ui.label(
                        RichText::new(&cd_info.title)
                            .strong()
                            .size(17.0)
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(&cd_info.artist)
                            .size(13.5)
                            .color(Color32::from_rgb(150, 170, 195)),
                    );

                    ui.add_space(6.0);

                    // Metadata Provider Badge
                    let provider_name = cd_info
                        .metadata_provider
                        .as_deref()
                        .unwrap_or("MusicBrainz");
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(12, 45, 48))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 160)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("✔ {provider_name} {}", self.lang.verified()))
                                    .size(11.0)
                                    .color(Color32::from_rgb(0, 230, 210))
                                    .strong(),
                            );
                        });
                } else {
                    ui.label(
                        RichText::new(self.lang.no_cd_detected())
                            .strong()
                            .size(16.0)
                            .color(Color32::from_rgb(160, 175, 195)),
                    );
                    ui.label(
                        RichText::new(self.lang.insert_cd_prompt())
                            .size(12.5)
                            .color(Color32::from_rgb(100, 115, 135)),
                    );
                }


                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // 3. Output Format Pills (FLAC, Opus, MP3, WAV, Ogg)
                ui.label(
                    RichText::new(self.lang.format_and_quality())
                        .size(12.0)
                        .color(Color32::from_rgb(140, 155, 175)),
                );
                ui.add_space(6.0);

                let formats = [
                    ("FLAC", 0, "flac"),
                    ("Opus", 1, "opus"),
                    ("MP3", 2, "mp3"),
                    ("WAV", 3, "wav"),
                    ("Ogg", 4, "ogg"),
                ];

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for (label, idx, enc_key) in formats {
                        let selected = self.format_index == idx;
                        let available = crate::ripper::check_encoder_tool_available(enc_key).is_ok();
                        let text = if available || enc_key == "wav" || enc_key == "flac" {
                            label.to_string()
                        } else {
                            format!("{label} ⚠️")
                        };

                        let btn = egui::Button::new(
                            RichText::new(text)
                                .size(12.0)
                                .color(if selected {
                                    Color32::from_rgb(10, 20, 25)
                                } else {
                                    Color32::from_rgb(190, 205, 225)
                                })
                                .strong(),
                        )
                        .fill(if selected {
                            Color32::from_rgb(0, 220, 200)
                        } else {
                            Color32::from_rgb(28, 36, 48)
                        })
                        .stroke(if selected {
                            Stroke::new(1.5, Color32::from_rgb(0, 240, 220))
                        } else {
                            Stroke::new(1.0, Color32::from_rgb(42, 54, 72))
                        })
                        .corner_radius(CornerRadius::same(12));

                        let mut response = ui.add(btn);
                        if !available && enc_key != "wav" && enc_key != "flac" {
                            response = response.on_hover_text(format!("Tool für {label} nicht im PATH gefunden (benötigt 'opusenc', 'lame', 'oggenc' oder 'ffmpeg')"));
                        }

                        if response.clicked() {
                            self.format_index = idx;
                        }
                    }
                });

                ui.add_space(12.0);

                // 4. Audio Bitrate / Quality Selector
                ui.label(
                    RichText::new(self.lang.format_and_quality())
                        .size(12.0)
                        .color(Color32::from_rgb(140, 155, 175)),
                );
                ui.add_space(4.0);

                let bitrate_labels = ["128 kbps (Standard)", "192 kbps (High)", "256 kbps (Very High)", "320 kbps (Maximum)"];
                egui::ComboBox::from_id_salt("bitrate_combo")
                    .width(sidebar_width - 32.0)
                    .selected_text(bitrate_labels[self.bitrate_index.min(3)])
                    .show_ui(ui, |ui| {
                        for (i, label) in bitrate_labels.iter().enumerate() {
                            ui.selectable_value(&mut self.bitrate_index, i, *label);
                        }
                    });

                ui.add_space(12.0);

                // 5. Output Directory Path
                ui.label(
                    RichText::new(self.lang.output_dir())
                        .size(12.0)
                        .color(Color32::from_rgb(140, 155, 175)),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.output_dir)
                        .desired_width(sidebar_width - 32.0)
                        .margin(egui::Margin::symmetric(8, 6)),
                );

                ui.add_space(18.0);

                // 6. Glowing Primary "Start Rip" CTA Button
                let is_active = self.is_ripping;
                let btn_text = if is_active {
                    RichText::new(format!("⏹ {}", self.lang.cancel_ripping()))
                        .size(15.0)
                        .color(Color32::WHITE)
                        .strong()
                } else {
                    RichText::new(format!("▶ {}", self.lang.start_ripping()))
                        .size(15.0)
                        .color(Color32::from_rgb(10, 22, 28))
                        .strong()
                };

                let btn_fill = if is_active {
                    Color32::from_rgb(225, 45, 60) // Red
                } else {
                    Color32::from_rgb(0, 220, 200) // Vibrant Cyan
                };

                let rip_btn = egui::Button::new(btn_text)
                    .fill(btn_fill)
                    .stroke(if is_active {
                        Stroke::new(1.0, Color32::from_rgb(255, 80, 95))
                    } else {
                        Stroke::new(1.5, Color32::from_rgb(20, 240, 220))
                    })
                    .corner_radius(CornerRadius::same(10))
                    .min_size(Vec2::new(sidebar_width - 32.0, 42.0));

                if ui.add(rip_btn).clicked() {
                    if self.is_ripping {
                        self.stop_ripping();
                    } else {
                        self.start_ripping();
                    }
                }

                if !self.status.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&self.status)
                            .size(11.5)
                            .color(Color32::from_rgb(160, 185, 210)),
                    );
                }
            });
    }

    fn render_cover_art_box(&self, ui: &mut egui::Ui, size: Vec2) {
        if let Some(texture) = &self.album_cover_texture {
            ui.add(
                egui::Image::new(texture)
                    .fit_to_exact_size(size)
                    .corner_radius(CornerRadius::same(12)),
            );
        } else {
            // Elegant Dark Placeholder Card with stylized concentric vinyl circles
            let (response, painter) = ui.allocate_painter(size, Sense::hover());
            let rect = response.rect;

            // Background card
            painter.rect_filled(rect, CornerRadius::same(12), Color32::from_rgb(28, 36, 48));
            painter.rect_stroke(
                rect,
                CornerRadius::same(12),
                Stroke::new(1.0, Color32::from_rgb(45, 58, 78)),
                egui::StrokeKind::Inside,
            );

            let center = rect.center();
            let max_r = (size.x * 0.38).min(size.y * 0.38);

            // Concentric vinyl rings
            for r_factor in [1.0, 0.82, 0.65, 0.48, 0.32] {
                painter.circle_stroke(
                    center,
                    max_r * r_factor,
                    Stroke::new(1.0, Color32::from_rgb(48, 62, 82)),
                );
            }

            // Center glow core
            painter.circle_filled(center, max_r * 0.22, Color32::from_rgb(0, 180, 165));
            painter.circle_filled(center, max_r * 0.08, Color32::from_rgb(16, 20, 26));

            // Music note
            painter.text(
                Pos2::new(center.x, rect.max.y - 20.0),
                egui::Align2::CENTER_CENTER,
                "Cover Art Preview",
                egui::FontId::proportional(11.0),
                Color32::from_rgb(120, 140, 165),
            );
        }
    }

    fn render_tracklist_and_progress(&mut self, ui: &mut egui::Ui, available_width: f32) {
        let total_tracks = self.cd_info.as_ref().map(|c| c.tracks.len()).unwrap_or(0);

        egui::Frame::NONE
            .fill(Color32::from_rgb(20, 26, 35))
            .stroke(Stroke::new(1.0, Color32::from_rgb(34, 44, 58)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(available_width - 32.0);

                // Table Header Row
                ui.horizontal(|ui| {
                    // Select All Checkbox
                    let all_selected = total_tracks > 0 && self.selected_tracks.iter().all(|&v| v);
                    let mut select_all = all_selected;
                    if ui.checkbox(&mut select_all, "").changed() {
                        for s in &mut self.selected_tracks {
                            *s = select_all;
                        }
                    }

                    ui.label(
                        RichText::new("#")
                            .size(12.0)
                            .color(Color32::from_rgb(120, 140, 165))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(self.lang.col_title())
                            .size(12.0)
                            .color(Color32::from_rgb(120, 140, 165))
                            .strong(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(self.lang.col_status())
                                .size(12.0)
                                .color(Color32::from_rgb(120, 140, 165))
                                .strong(),
                        );
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new("Waveform")
                                .size(12.0)
                                .color(Color32::from_rgb(120, 140, 165))
                                .strong(),
                        );
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new(self.lang.col_duration())
                                .size(12.0)
                                .color(Color32::from_rgb(120, 140, 165))
                                .strong(),
                        );
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Scrollable Track Rows
                let scroll_height = ui.available_height() - 75.0; // Reserve space for bottom progress bar
                egui::ScrollArea::vertical()
                    .id_salt("tracks_table_scroll")
                    .max_height(scroll_height.max(150.0))
                    .show(ui, |ui| {
                        ui.set_width(available_width - 32.0);

                        if let Some(cd_info) = &self.cd_info {
                            let tracks = cd_info.tracks.clone();
                            let toc = cd_info.toc.clone();

                            for (index, title) in tracks.iter().enumerate() {
                                let track_num = index + 1;
                                let is_selected =
                                    self.selected_tracks.get(index).copied().unwrap_or(true);
                                let is_completed =
                                    self.completed_tracks.get(index).copied().unwrap_or(false);
                                let is_active = self.is_ripping
                                    && self.current_ripping_track == Some(track_num);

                                // Format track duration from TOC sectors
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
                                    "03:45".to_string()
                                };

                                self.render_track_row(
                                    ui,
                                    index,
                                    track_num,
                                    title,
                                    &duration_str,
                                    is_selected,
                                    is_completed,
                                    is_active,
                                );
                                ui.add_space(4.0);
                            }
                        } else {
                            ui.vertical_centered(|ui| {
                                ui.add_space(36.0);
                                ui.label(
                                    RichText::new(format!("💿 {}", self.lang.no_cd_detected()))
                                        .strong()
                                        .size(15.0)
                                        .color(Color32::from_rgb(180, 200, 225)),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(self.lang.insert_cd_prompt())
                                        .size(12.5)
                                        .color(Color32::from_rgb(130, 150, 175)),
                                );
                                ui.add_space(14.0);

                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 6.0);
                                    let load_btn = egui::Button::new(
                                        RichText::new(format!("📥 {}", self.lang.load_tray()))
                                            .strong()
                                            .size(13.0),
                                    )
                                    .fill(Color32::from_rgb(0, 200, 180))
                                    .corner_radius(CornerRadius::same(8))
                                    .min_size(Vec2::new(200.0, 32.0));

                                    if ui.add(load_btn).clicked() {
                                        self.load_disc();
                                    }

                                    let mock_btn = egui::Button::new(
                                        RichText::new("💿 Virtuelle Test-CD laden").size(13.0),
                                    )
                                    .fill(Color32::from_rgb(28, 36, 48))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(42, 56, 76)))
                                    .corner_radius(CornerRadius::same(8))
                                    .min_size(Vec2::new(170.0, 32.0));

                                    if ui.add(mock_btn).clicked() {
                                        unsafe {
                                            std::env::set_var("CD_DEVICE", "mock");
                                        }
                                        self.detect_cd();
                                    }
                                });
                            });
                        }
                    });

                // Bottom Master Progress & Transport Status Bar
                ui.add_space(10.0);
                self.render_bottom_master_progress(ui, available_width - 32.0);
            });
    }

    fn render_track_row(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        track_num: usize,
        title: &str,
        duration_str: &str,
        is_selected: bool,
        is_completed: bool,
        is_active: bool,
    ) {
        let row_bg = if is_active {
            Color32::from_rgb(18, 42, 48) // Active cyan tint
        } else if index % 2 == 0 {
            Color32::from_rgb(24, 31, 42) // Alternating row
        } else {
            Color32::from_rgb(21, 28, 38)
        };

        let row_stroke = if is_active {
            Stroke::new(1.0, Color32::from_rgb(0, 200, 180))
        } else {
            Stroke::new(1.0, Color32::from_rgb(32, 42, 56))
        };

        egui::Frame::NONE
            .fill(row_bg)
            .stroke(row_stroke)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Checkbox
                    if let Some(selected_ref) = self.selected_tracks.get_mut(index) {
                        ui.checkbox(selected_ref, "");
                    }

                    // Track number
                    ui.label(
                        RichText::new(format!("{:02}", track_num))
                            .size(13.0)
                            .color(if is_active {
                                Color32::from_rgb(0, 230, 210)
                            } else {
                                Color32::from_rgb(140, 160, 185)
                            })
                            .strong(),
                    );

                    ui.add_space(6.0);

                    // Track title
                    let title_color = if !is_selected {
                        Color32::from_rgb(100, 115, 135)
                    } else if is_active {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(225, 235, 248)
                    };

                    ui.label(RichText::new(title).size(13.5).color(title_color).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status badge / Progress bar on the right
                        if is_completed {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(16, 48, 28))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(34, 197, 94)))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("{} ✔", self.lang.status_done()))
                                            .size(11.0)
                                            .color(Color32::from_rgb(74, 222, 128))
                                            .strong(),
                                    );
                                });
                        } else if is_active {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{} (32x)", self.lang.status_ripping()))
                                        .size(11.0)
                                        .color(Color32::from_rgb(0, 220, 200)),
                                );
                                let progress = self.track_progress_fraction.clamp(0.0, 1.0) as f32;
                                ui.add(
                                    egui::ProgressBar::new(progress)
                                        .show_percentage()
                                        .desired_width(110.0),
                                );
                            });
                        } else if !is_selected {
                            ui.label(
                                RichText::new(self.lang.status_skipped())
                                    .size(11.5)
                                    .color(Color32::from_rgb(90, 105, 120)),
                            );
                        } else {
                            ui.label(
                                RichText::new(self.lang.status_ready())
                                    .size(11.5)
                                    .color(Color32::from_rgb(130, 150, 175)),
                            );
                        }

                        ui.add_space(20.0);

                        // Stylized Audio Waveform Graphic
                        self.render_mini_waveform(ui, track_num, is_active, is_completed);

                        ui.add_space(20.0);

                        // Duration string
                        ui.label(
                            RichText::new(duration_str)
                                .size(13.0)
                                .color(Color32::from_rgb(150, 170, 195)),
                        );
                    });
                });
            });
    }

    fn render_mini_waveform(
        &self,
        ui: &mut egui::Ui,
        track_num: usize,
        is_active: bool,
        is_completed: bool,
    ) {
        let (response, painter) = ui.allocate_painter(Vec2::new(100.0, 20.0), Sense::hover());
        let rect = response.rect;
        let num_bars = 20;
        let bar_width = 3.0;
        let spacing = (rect.width() - (num_bars as f32 * bar_width)) / ((num_bars - 1) as f32);

        for i in 0..num_bars {
            let x = rect.min.x + (i as f32) * (bar_width + spacing);
            let phase = (i as f32) * 0.42 + (track_num as f32) * 1.5;
            let factor =
                (phase.sin().abs() * 0.65 + (phase * 2.2).cos().abs() * 0.35).clamp(0.2, 1.0);
            let bar_h = (rect.height() * factor).max(3.0);
            let y_min = rect.center().y - bar_h * 0.5;
            let y_max = rect.center().y + bar_h * 0.5;

            let color = if is_active {
                Color32::from_rgb(0, 230, 210) // Electric cyan
            } else if is_completed {
                Color32::from_rgb(45, 185, 95) // Green
            } else {
                Color32::from_rgb(52, 66, 86) // Subdued blue-gray
            };

            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x, y_min), Pos2::new(x + bar_width, y_max)),
                CornerRadius::same(2),
                color,
            );
        }
    }

    fn render_bottom_master_progress(&self, ui: &mut egui::Ui, width: f32) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(16, 21, 29))
            .stroke(Stroke::new(1.0, Color32::from_rgb(32, 42, 56)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    // Elapsed time
                    let elapsed_secs = self
                        .rip_start_time
                        .map(|t| t.elapsed().as_secs())
                        .unwrap_or(0);
                    ui.label(
                        RichText::new(format!(
                            "{} {:02}:{:02}",
                            self.lang.elapsed(),
                            elapsed_secs / 60,
                            elapsed_secs % 60
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(140, 160, 185)),
                    );

                    ui.add_space(8.0);

                    // Master Progress Bar
                    let master_progress = self.overall_progress_fraction.clamp(0.0, 1.0) as f32;
                    let bar_width = (width - 260.0).max(120.0);
                    ui.add(
                        egui::ProgressBar::new(master_progress)
                            .desired_width(bar_width)
                            .desired_height(8.0),
                    );

                    ui.add_space(8.0);

                    // ETA
                    let eta_text = if self.is_ripping && master_progress > 0.05 {
                        let total_est = (elapsed_secs as f32 / master_progress) as u64;
                        let remain = total_est.saturating_sub(elapsed_secs);
                        format!("ETA {:02}:{:02}", remain / 60, remain % 60)
                    } else if master_progress >= 1.0 {
                        self.lang.status_done().to_string()
                    } else {
                        "ETA --:--".to_string()
                    };

                    ui.label(
                        RichText::new(eta_text)
                            .size(12.0)
                            .color(Color32::from_rgb(140, 160, 185)),
                    );
                });
            });
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.lang.tab_settings());
        ui.add_space(10.0);

        egui::Frame::NONE
            .fill(Color32::from_rgb(20, 26, 35))
            .stroke(Stroke::new(1.0, Color32::from_rgb(34, 44, 58)))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                // Language Selection
                ui.horizontal(|ui| {
                    ui.label(self.lang.settings_language());
                    let current_lang_text = match self.language_index {
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
                        _ => "Auto",
                    };
                    egui::ComboBox::from_id_salt("settings_language_combo")
                        .selected_text(current_lang_text)
                        .show_ui(ui, |ui| {
                            let auto_label = format!("Auto ({})", Language::detect_system_language().display_name());
                            if ui.selectable_value(&mut self.language_index, 0, auto_label).changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 1, "English").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 2, "Deutsch").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 3, "Français").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 4, "Italiano").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 5, "Español").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 6, "Português").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 7, "日本語").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 8, "Русский").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 9, "Polski").changed() {
                                self.update_language_from_index();
                            }
                            if ui.selectable_value(&mut self.language_index, 10, "中文").changed() {
                                self.update_language_from_index();
                            }
                        });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(self.lang.settings_interface());
                    egui::ComboBox::from_id_salt("ui_backend")
                        .selected_text(match self.ui_backend_index {
                            1 => "TUI (Terminal)",
                            _ => "egui (Modern GUI)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.ui_backend_index, 0, "egui (Modern GUI)");
                            #[cfg(feature = "tui")]
                            ui.selectable_value(&mut self.ui_backend_index, 1, "TUI (Terminal)");
                        });
                });
                ui.label(RichText::new(self.lang.settings_restart_notice()).size(11.5).color(Color32::from_rgb(130, 145, 165)));
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(self.lang.settings_metadata_source());
                    egui::ComboBox::from_id_salt("metadata_source_settings")
                        .selected_text(match self.metadata_source_index {
                            1 => "MusicBrainz",
                            2 => "GnuDB (FreeDB)",
                            3 => "AcoustID (Acoustic)",
                            _ => "Auto (All)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.metadata_source_index, 0, "Auto (All)");
                            ui.selectable_value(&mut self.metadata_source_index, 1, "MusicBrainz");
                            ui.selectable_value(&mut self.metadata_source_index, 2, "GnuDB (FreeDB)");
                            ui.selectable_value(&mut self.metadata_source_index, 3, "AcoustID (Acoustic)");
                        });
                });
                ui.label(RichText::new("Auto cascades: MusicBrainz (TOC) -> GnuDB (FreeDB) -> AcoustID (Acoustic Fingerprinting).").size(11.5).color(Color32::from_rgb(130, 145, 165)));
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                let previous_size_index = self.album_art_size_index;
                ui.horizontal(|ui| {
                    ui.label(self.lang.settings_cover_size());
                    egui::ComboBox::from_id_salt("album_art_size")
                        .selected_text(match self.album_art_size_index {
                            1 => "Small",
                            2 => "Large",
                            3 => "Original",
                            _ => "Auto",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.album_art_size_index, 0, "Auto");
                            ui.selectable_value(&mut self.album_art_size_index, 1, "Small");
                            ui.selectable_value(&mut self.album_art_size_index, 2, "Large");
                            ui.selectable_value(&mut self.album_art_size_index, 3, "Original");
                        });
                });

                if previous_size_index != self.album_art_size_index {
                    self.reset_album_cover_state();
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Directory Auto-Creation Toggle
                ui.checkbox(
                    &mut self.auto_create_output_dir,
                    RichText::new(self.lang.settings_auto_create_dir())
                        .size(13.0)
                        .color(Color32::from_rgb(220, 235, 250)),
                );

                ui.add_space(16.0);
                if ui.button(RichText::new(format!("💾 {}", self.lang.settings_save())).strong()).clicked() {
                    self.save_ui_settings();
                }
            });
    }

    fn detect_cd(&mut self) {
        let metadata_source = self.metadata_source_key();
        match CdReader::detect_with_metadata_source(metadata_source) {
            Ok(cd_info) => {
                let track_count = cd_info.tracks.len();
                self.status = if let Some(err) = &cd_info.metadata_error {
                    format!("{}. {err}", self.lang.detected_tracks(track_count))
                } else if let Some(provider) = &cd_info.metadata_provider {
                    self.lang
                        .detected_tracks_with_provider(track_count, provider)
                } else {
                    self.lang.detected_tracks(track_count)
                };
                self.selected_tracks = vec![true; track_count];
                self.completed_tracks = vec![false; track_count];
                self.reset_album_cover_state();
                self.cd_info = Some(cd_info);
                self.is_tray_ejected = false;
            }
            Err(err) => {
                self.status = format!("Keine CD erkannt ({err})");
                self.selected_tracks.clear();
                self.completed_tracks.clear();
                self.reset_album_cover_state();
                self.cd_info = None;
            }
        }
    }

    fn maybe_auto_detect_cd(&mut self, ctx: &egui::Context) {
        if self.cd_info.is_some() || self.is_ripping || self.is_tray_ejected {
            return;
        }

        let now = Instant::now();
        if now < self.next_auto_detect_at {
            ctx.request_repaint_after(self.next_auto_detect_at - now);
            return;
        }

        self.next_auto_detect_at = now + Self::AUTO_DETECT_RETRY_INTERVAL;
        self.detect_cd();

        if self.cd_info.is_none() {
            ctx.request_repaint_after(Self::AUTO_DETECT_RETRY_INTERVAL);
        }
    }

    fn ensure_album_cover(&mut self, ctx: &egui::Context, cover_url: Option<&str>) {
        let Some(url) = cover_url else {
            self.album_cover_texture = None;
            self.album_cover_source_url = None;
            self.album_cover_error = None;
            return;
        };

        if self.album_cover_source_url.as_deref() == Some(url)
            && (self.album_cover_texture.is_some() || self.album_cover_error.is_some())
        {
            return;
        }

        self.album_cover_source_url = Some(url.to_string());
        self.album_cover_texture = None;
        self.album_cover_error = None;

        match Self::download_album_cover_texture(ctx, url) {
            Ok(texture) => {
                self.album_cover_texture = Some(texture);
            }
            Err(err) => {
                self.album_cover_error = Some(err);
            }
        }
    }

    fn download_album_cover_texture(
        ctx: &egui::Context,
        url: &str,
    ) -> Result<egui::TextureHandle, String> {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .new_agent();
        let mut response = agent
            .get(url)
            .call()
            .map_err(|err| format!("Failed to fetch album cover: {err}"))?;

        let bytes = response
            .body_mut()
            .with_config()
            .limit(50 * 1024 * 1024)
            .read_to_vec()
            .map_err(|err| format!("Failed to read album cover bytes: {err}"))?;

        let image = image::load_from_memory(&bytes)
            .map_err(|err| format!("Failed to decode album cover: {err}"))?
            .to_rgba8();

        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());

        Ok(ctx.load_texture("album-cover", color_image, egui::TextureOptions::LINEAR))
    }

    fn save_ui_settings(&mut self) {
        let mut cfg = Config::load();
        cfg.language = self.language_key().to_string();
        cfg.encoder = self.encoder_key().to_string();
        cfg.bitrate = self.bitrate_key().to_string();
        cfg.ui_backend = self.ui_backend_key().to_string();
        cfg.metadata_source = self.metadata_source_key().to_string();
        cfg.album_art_size_preference = self.album_art_size_key().to_string();
        cfg.album_art_download_behavior = self.album_art_download_key().to_string();
        cfg.auto_create_output_dir = self.auto_create_output_dir;
        match cfg.save() {
            Ok(()) => {
                self.reset_album_cover_state();
                self.status = self.lang.settings_saved().to_string();
            }
            Err(err) => self.status = format!("Failed to save settings: {err}"),
        }
    }

    fn start_ripping(&mut self) {
        if self.cd_info.is_none() {
            self.status = "No CD detected.".to_string();
            return;
        }

        let selected_count = self.selected_tracks.iter().copied().filter(|v| *v).count();
        if selected_count == 0 {
            self.status = "No tracks selected. Please select at least one track.".to_string();
            return;
        }

        let trimmed = self.output_dir.trim();
        if trimmed.is_empty() {
            self.status = "Output directory is empty.".to_string();
            return;
        }

        let enc_key = self.encoder_key();
        if crate::ripper::check_encoder_tool_available(enc_key).is_err() {
            self.missing_encoder_name = enc_key.to_string();
            self.show_missing_encoder_modal = true;
            return;
        }

        let output_path = PathBuf::from(trimmed);
        if !output_path.exists() && !self.auto_create_output_dir {
            self.pending_create_dir_path = Some(output_path);
            self.dont_ask_create_dir_checkbox = false;
            self.show_create_dir_modal = true;
            return;
        }

        self.execute_start_ripping();
    }

    fn execute_start_ripping(&mut self) {
        let output_dir = PathBuf::from(self.output_dir.trim());
        let _ = std::fs::create_dir_all(&output_dir);

        let mut cfg = Config::load();
        cfg.language = self.language_key().to_string();
        cfg.encoder = self.encoder_key().to_string();
        cfg.bitrate = self.bitrate_key().to_string();
        cfg.ui_backend = self.ui_backend_key().to_string();
        cfg.metadata_source = self.metadata_source_key().to_string();
        cfg.album_art_size_preference = self.album_art_size_key().to_string();
        cfg.album_art_download_behavior = self.album_art_download_key().to_string();
        cfg.auto_create_output_dir = self.auto_create_output_dir;
        let _ = cfg.save();

        let Some(mut selected_cd) = self.cd_info.clone() else {
            self.status = "No CD detected to rip.".to_string();
            return;
        };
        selected_cd.album_cover_url = self.current_album_cover_url();
        selected_cd.tracks = selected_cd
            .tracks
            .into_iter()
            .zip(self.selected_tracks.iter().copied())
            .filter_map(|(track, include)| include.then_some(track))
            .collect();

        let (sender, receiver) = mpsc::unbounded_channel();
        let ripper = match Ripper::new(cfg, output_dir, sender) {
            Ok(r) => Arc::new(r),
            Err(e) => {
                self.status = format!("Failed to initialize ripper: {e}");
                return;
            }
        };

        self.overall_progress_fraction = 0.0;
        self.track_progress_fraction = 0.0;
        self.current_ripping_track = Some(1);
        self.completed_tracks = vec![false; self.selected_tracks.len()];
        self.progress_label = "Starting...".to_string();
        self.status = "Ripping started.".to_string();
        self.rip_start_time = Some(Instant::now());
        self.is_ripping = true;
        self.ripper = Some(ripper.clone());
        self.rip_receiver = Some(receiver);

        std::thread::spawn(move || {
            ripper.rip(&selected_cd);
        });
    }

    fn render_create_dir_modal(&mut self, ctx: &egui::Context) {
        if !self.show_create_dir_modal {
            return;
        }

        let dir_str = self
            .pending_create_dir_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| self.output_dir.clone());

        let mut close_modal = false;
        let mut proceed_rip = false;

        egui::Modal::new(egui::Id::new("create_dir_modal"))
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_rgb(22, 28, 38))
                    .stroke(Stroke::new(1.5, Color32::from_rgb(0, 200, 180)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(450.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("📁").size(26.0));
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(self.lang.create_dir_title())
                                .strong()
                                .size(16.5)
                                .color(Color32::WHITE),
                        );
                    });
                });

                ui.add_space(14.0);

                // Directory path box
                egui::Frame::NONE
                    .fill(Color32::from_rgb(14, 18, 26))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(38, 48, 64)))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&dir_str)
                                .monospace()
                                .size(12.5)
                                .color(Color32::from_rgb(0, 230, 210)),
                        );
                    });

                ui.add_space(12.0);
                ui.label(
                    RichText::new(self.lang.create_dir_prompt(&dir_str))
                        .size(13.0)
                        .color(Color32::from_rgb(220, 230, 245)),
                );

                ui.add_space(12.0);

                // Checkbox: Do not ask again
                ui.checkbox(
                    &mut self.dont_ask_create_dir_checkbox,
                    RichText::new(self.lang.settings_auto_create_dir())
                        .size(12.0)
                        .color(Color32::from_rgb(170, 190, 215)),
                );

                ui.add_space(18.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new(self.lang.cancel()).size(13.0))
                                .fill(Color32::from_rgb(32, 40, 52))
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(100.0, 34.0)),
                        )
                        .clicked()
                    {
                        close_modal = true;
                        self.status = "Cancelled.".to_string();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("✔ {}", self.lang.create_dir_confirm()))
                                        .strong()
                                        .size(13.0)
                                        .color(Color32::from_rgb(10, 22, 28)),
                                )
                                .fill(Color32::from_rgb(0, 220, 200))
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(160.0, 34.0)),
                            )
                            .clicked()
                        {
                            proceed_rip = true;
                        }
                    });
                });
            });

        if close_modal {
            self.show_create_dir_modal = false;
            self.pending_create_dir_path = None;
        } else if proceed_rip {
            if self.dont_ask_create_dir_checkbox {
                self.auto_create_output_dir = true;
                let mut cfg = Config::load();
                cfg.auto_create_output_dir = true;
                let _ = cfg.save();
            }
            self.show_create_dir_modal = false;
            self.pending_create_dir_path = None;
            self.execute_start_ripping();
        }
    }

    fn render_missing_encoder_modal(&mut self, ctx: &egui::Context) {
        if !self.show_missing_encoder_modal {
            return;
        }

        let mut should_close = false;
        let mut switch_to_flac = false;
        let mut switch_to_wav = false;

        let modal = egui::Modal::new(egui::Id::new("missing_encoder_modal"));
        modal.show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.vertical(|ui| {
                ui.heading(
                    RichText::new(format!("⚠️ {}", self.lang.missing_encoder_title()))
                        .color(Color32::from_rgb(255, 180, 50))
                        .size(17.0),
                );
                ui.add_space(8.0);

                let format_upper = self.missing_encoder_name.to_uppercase();
                ui.label(
                    RichText::new(self.lang.missing_encoder_prompt(&format_upper))
                        .size(13.0)
                        .color(Color32::from_rgb(220, 230, 240)),
                );

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);

                    let flac_btn = egui::Button::new(
                        RichText::new(format!("🎵 {}", self.lang.rip_as_flac()))
                            .color(Color32::from_rgb(10, 20, 25))
                            .strong(),
                    )
                    .fill(Color32::from_rgb(0, 220, 200))
                    .corner_radius(CornerRadius::same(8));

                    if ui.add(flac_btn).clicked() {
                        switch_to_flac = true;
                    }

                    let wav_btn = egui::Button::new(RichText::new(self.lang.rip_as_wav()))
                        .fill(Color32::from_rgb(28, 38, 52))
                        .corner_radius(CornerRadius::same(8));

                    if ui.add(wav_btn).clicked() {
                        switch_to_wav = true;
                    }

                    let cancel_btn = egui::Button::new(RichText::new(self.lang.cancel()))
                        .fill(Color32::from_rgb(34, 40, 50))
                        .corner_radius(CornerRadius::same(8));

                    if ui.add(cancel_btn).clicked() {
                        should_close = true;
                    }
                });
            });
        });

        if should_close {
            self.show_missing_encoder_modal = false;
        } else if switch_to_flac {
            self.format_index = 0; // FLAC
            self.show_missing_encoder_modal = false;
            self.start_ripping();
        } else if switch_to_wav {
            self.format_index = 3; // WAV
            self.show_missing_encoder_modal = false;
            self.start_ripping();
        }
    }

    fn stop_ripping(&mut self) {
        if let Some(ripper) = &self.ripper {
            ripper.cancel();
        }
        self.finish_ripping_state();
        self.overall_progress_fraction = 0.0;
        self.track_progress_fraction = 0.0;
        self.progress_label.clear();
        self.status = "Ripping stopped.".to_string();
    }

    fn eject_disc(&mut self) {
        #[cfg(windows)]
        {
            let dev = CdReader::active_device_path();
            match crate::windows_cd::eject_windows_drive(&dev) {
                Ok(()) => {
                    self.status = "CD ausgeworfen (Laufwerk geöffnet).".to_string();
                    self.cd_info = None;
                    self.selected_tracks.clear();
                    self.completed_tracks.clear();
                    self.reset_album_cover_state();
                    self.is_tray_ejected = true;
                    self.next_auto_detect_at = Instant::now() + Duration::from_secs(5);
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
                    self.reset_album_cover_state();
                    self.is_tray_ejected = true;
                    self.next_auto_detect_at = Instant::now() + Duration::from_secs(5);
                }
                Ok(_) => {
                    self.status =
                        "Eject-Befehl fehlgeschlagen. Bitte sicherstellen, dass 'eject' installiert ist."
                            .to_string();
                }
                Err(err) => {
                    self.status = format!("Konnte 'eject' nicht ausführen: {err}");
                }
            }
        }
    }

    fn load_disc(&mut self) {
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
                }
                Err(err) => {
                    self.status = format!("Konnte 'eject -t' nicht ausführen: {err}");
                }
            }
        }
    }

    fn poll_ripper_messages(&mut self) {
        let mut disconnected = false;
        let mut messages = Vec::new();

        if let Some(receiver) = self.rip_receiver.as_mut() {
            loop {
                match receiver.try_recv() {
                    Ok(msg) => messages.push(msg),
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        for msg in messages {
            self.handle_rip_message(msg);
        }

        if disconnected {
            self.rip_receiver = None;
            if self.is_ripping {
                self.finish_ripping_state();
            }
        }
    }

    fn handle_rip_message(&mut self, msg: RipMessage) {
        match msg {
            RipMessage::Progress(fraction, message) => {
                self.overall_progress_fraction = fraction;
                self.progress_label = message.clone();

                // Extract track number from message e.g. "Ripping track 3 of 10 (45%)..."
                if let Some(rest) = message.strip_prefix("Ripping track ") {
                    if let Some((num_str, _)) = rest.split_once(' ') {
                        if let Ok(num) = num_str.parse::<usize>() {
                            self.current_ripping_track = Some(num);
                        }
                    }
                    if let Some(pct_str) =
                        message.split('(').nth(1).and_then(|s| s.split('%').next())
                    {
                        if let Ok(pct) = pct_str.parse::<f64>() {
                            self.track_progress_fraction = pct / 100.0;
                        }
                    }
                }
            }
            RipMessage::TrackComplete(track_num) => {
                if track_num > 0 && track_num <= self.completed_tracks.len() {
                    self.completed_tracks[track_num - 1] = true;
                }
                self.status = format!("Finished track {}", track_num);
            }
            RipMessage::Success => {
                self.finish_ripping_state();
                self.overall_progress_fraction = 1.0;
                self.track_progress_fraction = 1.0;
                for c in &mut self.completed_tracks {
                    *c = true;
                }
                self.progress_label = "Completed".to_string();
                self.status = "CD ripped successfully!".to_string();
            }
            RipMessage::Error(err) => {
                self.finish_ripping_state();
                self.status = format!("Ripping failed: {err}");
            }
        }
    }

    fn finish_ripping_state(&mut self) {
        self.is_ripping = false;
        self.ripper = None;
        self.rip_receiver = None;
    }

    fn reset_album_cover_state(&mut self) {
        self.album_cover_texture = None;
        self.album_cover_source_url = None;
        self.album_cover_error = None;
    }

    fn current_album_cover_url(&self) -> Option<String> {
        let cd_info = self.cd_info.as_ref()?;
        let preference = self.album_art_size_key();
        cd_info
            .preferred_album_cover_url(preference)
            .map(ToOwned::to_owned)
    }

    fn encoder_key(&self) -> &'static str {
        match self.format_index {
            1 => "opus",
            2 => "mp3",
            3 => "wav",
            4 => "ogg",
            _ => "flac",
        }
    }

    fn bitrate_key(&self) -> &'static str {
        match self.bitrate_index {
            0 => "128",
            1 => "192",
            2 => "256",
            _ => "320",
        }
    }

    fn ui_backend_key(&self) -> &'static str {
        match self.ui_backend_index {
            1 => "tui",
            _ => "egui",
        }
    }

    fn metadata_source_key(&self) -> &'static str {
        match self.metadata_source_index {
            1 => "musicbrainz",
            2 => "gnudb",
            3 => "acoustid",
            _ => "auto",
        }
    }

    fn album_art_size_key(&self) -> &'static str {
        match self.album_art_size_index {
            1 => "small",
            2 => "large",
            3 => "original",
            _ => "auto",
        }
    }

    fn album_art_download_key(&self) -> &'static str {
        match self.album_art_download_index {
            1 => "save-with-rip",
            _ => "preview-only",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cjk_fonts_discovery_and_egui_layout() {
        let paths = find_system_cjk_font_paths();
        println!("Discovered CJK font paths: {:?}", paths);

        let ctx = egui::Context::default();
        setup_custom_fonts(&ctx);

        let mut ja_width = 0.0;
        let mut zh_width = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ctx| {
            ja_width = ctx.fonts_mut(|f| f.glyph_width(&egui::FontId::proportional(14.0), '日'));
            zh_width = ctx.fonts_mut(|f| f.glyph_width(&egui::FontId::proportional(14.0), '抓'));
        });
        out.textures_delta.clear();

        if !paths.is_empty() {
            assert!(
                ja_width > 0.0,
                "Japanese glyph '日' must have positive width"
            );
            assert!(
                zh_width > 0.0,
                "Chinese glyph '抓' must have positive width"
            );
        }
    }
}
