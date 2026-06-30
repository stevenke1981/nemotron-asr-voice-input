use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use super::state::{GuiAction, GuiSnapshot, ModelStatus};
use crate::config::AppConfig;
use crate::ui::strings::{Strings, UiLang};

// ═══════════════════════════════════════════════════════════════════
// Design Tokens (Color System)
// ═══════════════════════════════════════════════════════════════════

mod color {
    use egui::Color32;

    // ── Dark Theme ────────────────────────────────────────────────
    #[allow(dead_code)]
    pub mod dark {
        use super::*;
        pub const BG: Color32 = Color32::from_rgb(10, 12, 16);        // #0A0C10
        pub const PANEL: Color32 = Color32::from_rgb(18, 20, 24);     // #121418
        pub const HOVER: Color32 = Color32::from_rgb(26, 28, 34);     // #1A1C22
        pub const BORDER: Color32 = Color32::from_rgb(42, 44, 48);    // #2A2C30
        pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(232, 230, 225); // #E8E6E1
        pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 136); // #8C8C88
        pub const TEXT_MUTED: Color32 = Color32::from_rgb(90, 92, 96);     // #5A5C60
        pub const ACCENT: Color32 = Color32::from_rgb(107, 138, 255);      // #6B8AFF
        pub const ACCENT_HOVER: Color32 = Color32::from_rgb(139, 164, 255); // #8BA4FF
        pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94);       // #22C55E
        pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68);         // #EF4444
        pub const RECORDING: Color32 = Color32::from_rgb(239, 68, 68);     // #EF4444
        pub const CARD: Color32 = Color32::from_rgb(22, 24, 28);          // #16181C
    }

    // ── Light Theme ───────────────────────────────────────────────
    #[allow(dead_code)]
    pub mod light {
        use super::*;
        pub const BG: Color32 = Color32::from_rgb(247, 245, 242);    // #F7F5F2
        pub const PANEL: Color32 = Color32::from_rgb(255, 255, 255); // #FFFFFF
        pub const HOVER: Color32 = Color32::from_rgb(232, 230, 225); // #E8E6E1
        pub const BORDER: Color32 = Color32::from_rgb(209, 207, 200); // #D1CFC8
        pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(26, 28, 30);  // #1A1C1E
        pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(110, 108, 104); // #6E6C68
        pub const TEXT_MUTED: Color32 = Color32::from_rgb(160, 158, 152); // #A09E98
        pub const ACCENT: Color32 = Color32::from_rgb(74, 108, 247);     // #4A6CF7
        pub const ACCENT_HOVER: Color32 = Color32::from_rgb(59, 93, 231); // #3B5DE7
        pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94);     // #22C55E
        pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68);       // #EF4444
        pub const RECORDING: Color32 = Color32::from_rgb(239, 68, 68);   // #EF4444
        pub const CARD: Color32 = Color32::from_rgb(255, 255, 255);      // #FFFFFF
    }
}

// ── Theme helpers ────────────────────────────────────────────────

fn is_dark(theme: &str) -> bool {
    theme != "Light"
}

fn text_primary(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::TEXT_PRIMARY } else { color::light::TEXT_PRIMARY }
}

fn text_secondary(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::TEXT_SECONDARY } else { color::light::TEXT_SECONDARY }
}

fn text_muted(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::TEXT_MUTED } else { color::light::TEXT_MUTED }
}

fn accent_color(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::ACCENT } else { color::light::ACCENT }
}

fn panel_fill(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::PANEL } else { color::light::PANEL }
}

fn card_fill(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::CARD } else { color::light::CARD }
}

fn border_color(theme: &str) -> egui::Color32 {
    if is_dark(theme) { color::dark::BORDER } else { color::light::BORDER }
}

// ═══════════════════════════════════════════════════════════════════
// Custom egui Theme Setup
// ═══════════════════════════════════════════════════════════════════

fn apply_custom_theme(ctx: &egui::Context, theme_name: &str) {
    let dark = is_dark(theme_name);
    let mut style = (*ctx.style()).clone();

    if dark {
        style.visuals = egui::Visuals {
            dark_mode: true,
            window_fill: color::dark::BG,
            panel_fill: color::dark::PANEL,
            faint_bg_color: color::dark::HOVER,
            extreme_bg_color: color::dark::TEXT_PRIMARY,
            override_text_color: Some(color::dark::TEXT_PRIMARY),
            window_stroke: egui::Stroke::new(1.0, color::dark::BORDER),
            button_frame: true,
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    bg_fill: color::dark::HOVER,
                    weak_bg_fill: color::dark::BG,
                    fg_stroke: egui::Stroke::new(1.0, color::dark::TEXT_SECONDARY),
                    bg_stroke: egui::Stroke::new(1.0, color::dark::BORDER),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
                inactive: egui::style::WidgetVisuals {
                    bg_fill: color::dark::CARD,
                    weak_bg_fill: color::dark::CARD,
                    fg_stroke: egui::Stroke::new(1.5, color::dark::TEXT_PRIMARY),
                    bg_stroke: egui::Stroke::new(1.0, color::dark::BORDER),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    bg_fill: color::dark::HOVER,
                    weak_bg_fill: color::dark::HOVER,
                    fg_stroke: egui::Stroke::new(1.5, color::dark::TEXT_PRIMARY),
                    bg_stroke: egui::Stroke::new(1.0, color::dark::ACCENT),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 1.0,
                },
                active: egui::style::WidgetVisuals {
                    bg_fill: color::dark::ACCENT,
                    weak_bg_fill: color::dark::ACCENT,
                    fg_stroke: egui::Stroke::new(2.0, color::dark::TEXT_PRIMARY),
                    bg_stroke: egui::Stroke::new(1.0, color::dark::ACCENT),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
                open: egui::style::WidgetVisuals {
                    bg_fill: color::dark::HOVER,
                    weak_bg_fill: color::dark::HOVER,
                    fg_stroke: egui::Stroke::new(2.0, color::dark::ACCENT),
                    bg_stroke: egui::Stroke::new(1.0, color::dark::ACCENT),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
            },
            selection: egui::style::Selection {
                bg_fill: egui::Color32::from_rgba_premultiplied(107, 138, 255, 96),
                stroke: egui::Stroke::new(1.0, color::dark::ACCENT),
            },
            hyperlink_color: color::dark::ACCENT,
            collapsing_header_frame: true,
            ..Default::default()
        };
    } else {
        style.visuals = egui::Visuals {
            dark_mode: false,
            window_fill: color::light::BG,
            panel_fill: color::light::PANEL,
            faint_bg_color: color::light::HOVER,
            extreme_bg_color: color::light::TEXT_PRIMARY,
            override_text_color: Some(color::light::TEXT_PRIMARY),
            window_stroke: egui::Stroke::new(1.0, color::light::BORDER),
            button_frame: true,
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    bg_fill: color::light::HOVER,
                    weak_bg_fill: color::light::BG,
                    fg_stroke: egui::Stroke::new(1.0, color::light::TEXT_SECONDARY),
                    bg_stroke: egui::Stroke::new(1.0, color::light::BORDER),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
                inactive: egui::style::WidgetVisuals {
                    bg_fill: color::light::PANEL,
                    weak_bg_fill: color::light::PANEL,
                    fg_stroke: egui::Stroke::new(1.5, color::light::TEXT_PRIMARY),
                    bg_stroke: egui::Stroke::new(1.0, color::light::BORDER),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    bg_fill: color::light::HOVER,
                    weak_bg_fill: color::light::HOVER,
                    fg_stroke: egui::Stroke::new(1.5, color::light::TEXT_PRIMARY),
                    bg_stroke: egui::Stroke::new(1.0, color::light::ACCENT),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 1.0,
                },
                active: egui::style::WidgetVisuals {
                    bg_fill: color::light::ACCENT,
                    weak_bg_fill: color::light::ACCENT,
                    fg_stroke: egui::Stroke::new(2.0, egui::Color32::WHITE),
                    bg_stroke: egui::Stroke::new(1.0, color::light::ACCENT),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
                open: egui::style::WidgetVisuals {
                    bg_fill: color::light::HOVER,
                    weak_bg_fill: color::light::HOVER,
                    fg_stroke: egui::Stroke::new(2.0, color::light::ACCENT),
                    bg_stroke: egui::Stroke::new(1.0, color::light::ACCENT),
                    corner_radius: egui::CornerRadius::same(6),
                    expansion: 0.0,
                },
            },
            selection: egui::style::Selection {
                bg_fill: egui::Color32::from_rgba_premultiplied(74, 108, 247, 64),
                stroke: egui::Stroke::new(1.0, color::light::ACCENT),
            },
            hyperlink_color: color::light::ACCENT,
            collapsing_header_frame: true,
            ..Default::default()
        };
    }

    style.spacing.item_spacing = egui::Vec2::new(12.0, 8.0);
    style.spacing.button_padding = egui::Vec2::new(16.0, 8.0);
    style.spacing.indent = 24.0;
    style.spacing.window_margin = egui::Margin::symmetric(12, 12);

    ctx.set_style(style);
}

// ── Font ─────────────────────────────────────────────────────────

/// Common Chinese font paths on Windows, in priority order.
const CHINESE_FONT_PATHS: &[&str] = &[
    "C:\\Windows\\Fonts\\msyh.ttc",   // Microsoft YaHei (Simplified)
    "C:\\Windows\\Fonts\\msjh.ttc",   // Microsoft JhengHei (Traditional)
    "C:\\Windows\\Fonts\\simsun.ttc", // SimSun (Song)
    "C:\\Windows\\Fonts\\simhei.ttf", // SimHei
    "C:\\Windows\\Fonts\\deng.ttf",   // DengXian
];

/// Set up CJK fonts once at startup (anti-pattern: never per-frame).
fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in CHINESE_FONT_PATHS {
        match std::fs::read(path) {
            Ok(data) => {
                let stem = std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("chinese_font");
                fonts.font_data
                    .insert(stem.to_owned(), Arc::new(egui::FontData::from_owned(data)));
                fonts.families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, stem.to_owned());
                tracing::info!("Loaded CJK font: {}", path);
                break;
            }
            Err(_) => continue,
        }
    }
    ctx.set_fonts(fonts);
}

// ═══════════════════════════════════════════════════════════════════
// Shared State
// ═══════════════════════════════════════════════════════════════════

pub struct GuiSharedState {
    #[allow(dead_code)]
    pub snapshot: Arc<Mutex<GuiSnapshot>>,
    pub gui_rx: crossbeam::channel::Receiver<GuiSnapshot>,
    pub action_tx: crossbeam::channel::Sender<GuiAction>,
    pub show_overlay: Arc<AtomicBool>,
    pub is_running: Arc<AtomicBool>,
    pub model_status: Arc<Mutex<ModelStatus>>,
}

// ═══════════════════════════════════════════════════════════════════
// GuiApp
// ═══════════════════════════════════════════════════════════════════

pub struct GuiApp {
    state: GuiSharedState,
    current_snapshot: GuiSnapshot,
    show_settings: bool,
    show_overlay_local: bool,
    model_ready: bool,
    // Settings
    settings_language: String,
    settings_provider: String,
    settings_num_threads: u32,
    settings_use_vad: bool,
    settings_vad_threshold: f32,
    settings_decoding_method: String,
    settings_inject_strategy: String,
    settings_key_delay_ms: u64,
    settings_restore_clipboard: bool,
    settings_conversion_mode: String,
    settings_ui_lang: String,
    settings_theme: String,
    // Window geometry
    window_x: f32,
    window_y: f32,
    window_w: f32,
    window_h: f32,
    /// Bilingual UI strings.
    ui_strings: Strings,
    /// Snapshot of the current AppConfig for preserving non-GUI fields on save.
    base_config: Option<AppConfig>,
}

impl GuiApp {
    pub fn new(
        state: GuiSharedState,
        initial_pos: Option<egui::Pos2>,
        initial_size: Option<egui::Vec2>,
        initial_theme: Option<String>,
        initial_lang: UiLang,
        initial_config: Option<&AppConfig>,
    ) -> Self {
        let (wx, wy) = initial_pos.map(|p| (p.x, p.y)).unwrap_or((100.0, 100.0));
        let (ww, wh) = initial_size.map(|s| (s.x, s.y)).unwrap_or((800.0, 600.0));
        let theme = initial_theme.unwrap_or_else(|| "Dark".into());

        let (settings_language, settings_provider, settings_num_threads,
             settings_use_vad, settings_vad_threshold, settings_decoding_method,
             settings_inject_strategy, settings_key_delay_ms, settings_restore_clipboard,
             settings_conversion_mode, settings_ui_lang) = match initial_config {
            Some(cfg) => (
                cfg.language.language.clone(),
                cfg.asr.provider.clone(),
                cfg.asr.num_threads,
                cfg.asr.use_vad,
                cfg.asr.vad_threshold,
                cfg.asr.decoding_method.clone(),
                cfg.injector.strategy.clone(),
                cfg.injector.key_delay_ms,
                cfg.injector.restore_clipboard,
                cfg.conversion.mode.clone(),
                cfg.ui.language.clone(),
            ),
            None => (
                "zh".into(), "cpu".into(), 4, true, 0.1,
                "greedy_search".into(), "auto".into(), 5, true,
                "s2t".into(),
                initial_lang.code().to_string(),
            ),
        };

        let model_ready = matches!(*state.model_status.lock().unwrap(), ModelStatus::Ready);

        Self {
            state,
            current_snapshot: GuiSnapshot::default(),
            base_config: initial_config.cloned(),
            show_settings: false,
            show_overlay_local: false,
            model_ready,
            settings_language,
            settings_provider,
            settings_num_threads,
            settings_use_vad,
            settings_vad_threshold,
            settings_decoding_method,
            settings_inject_strategy,
            settings_key_delay_ms,
            settings_restore_clipboard,
            settings_conversion_mode,
            settings_ui_lang,
            settings_theme: theme,
            window_x: wx,
            window_y: wy,
            window_w: ww,
            window_h: wh,
            ui_strings: Strings::new(initial_lang),
        }
    }

    fn process_incoming(&mut self) {
        while let Ok(snapshot) = self.state.gui_rx.try_recv() {
            if snapshot.show_settings_requested {
                self.show_settings = true;
            }
            self.current_snapshot = snapshot;
        }
    }

    fn send_action(&self, action: GuiAction) {
        let _ = self.state.action_tx.send(action);
    }

    // ── Startup panel (model download progress) ────────────────────

    fn show_startup_panel(&self, ctx: &egui::Context, status: &ModelStatus) {
        apply_custom_theme(ctx, &self.settings_theme);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered_justified(|ui: &mut egui::Ui| {
                ui.add_space(80.0);

                // App icon / branding
                ui.label(
                    egui::RichText::new("🎤")
                        .size(48.0),
                );
                ui.add_space(8.0);
                ui.heading(
                    egui::RichText::new(self.ui_strings.app_name())
                        .size(24.0)
                        .color(text_primary(&self.settings_theme)),
                );
                ui.add_space(24.0);

                match status {
                    ModelStatus::Checking => {
                        ui.label(
                            egui::RichText::new(self.ui_strings.startup_checking())
                                .size(14.0)
                                .color(text_secondary(&self.settings_theme)),
                        );
                        ui.add_space(12.0);
                        ui.spinner();
                    }
                    ModelStatus::Downloading(_current, _total) => {
                        ui.label(
                            egui::RichText::new(self.ui_strings.startup_downloading())
                                .size(14.0)
                                .color(text_secondary(&self.settings_theme)),
                        );
                        ui.add_space(12.0);
                        let progress = if *_total > 0 {
                            *_current as f32 / *_total as f32
                        } else {
                            0.0
                        };
                        let bar = egui::ProgressBar::new(progress.clamp(0.0, 1.0))
                            .show_percentage()
                            .desired_width(360.0)
                            .fill(accent_color(&self.settings_theme));
                        ui.add(bar);
                        ui.add_space(4.0);
                        let mb_c = *_current as f64 / 1_048_576.0;
                        let mb_t = *_total as f64 / 1_048_576.0;
                        if *_total > 0 {
                            ui.label(
                                egui::RichText::new(format!("{:.1} MB / {:.1} MB", mb_c, mb_t))
                                    .size(12.0)
                                    .color(text_muted(&self.settings_theme)),
                            );
                        }
                    }
                    ModelStatus::Extracting => {
                        ui.label(
                            egui::RichText::new(self.ui_strings.startup_extracting())
                                .size(14.0)
                                .color(text_secondary(&self.settings_theme)),
                        );
                        ui.add_space(12.0);
                        ui.spinner();
                    }
                    ModelStatus::Failed(msg) => {
                        ui.label(
                            egui::RichText::new(self.ui_strings.startup_failed())
                                .size(14.0)
                                .color(if is_dark(&self.settings_theme) { color::dark::ERROR } else { color::light::ERROR }),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(msg.as_str())
                                .size(12.0)
                                .color(text_secondary(&self.settings_theme)),
                        );
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.ui_strings.startup_retry()).clicked() {}
                            if ui.button(self.ui_strings.startup_continue_without_models()).clicked() {
                                *self.state.model_status.lock().unwrap() = ModelStatus::Ready;
                            }
                        });
                    }
                    ModelStatus::Ready => {}
                }

                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(self.ui_strings.startup_hint())
                        .size(11.0)
                        .color(text_muted(&self.settings_theme)),
                );
            });
        });
    }
}

// ═══════════════════════════════════════════════════════════════════
// eframe::App
// ═══════════════════════════════════════════════════════════════════

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Shutdown check ─────────────────────────────────────────
        if !self.state.is_running.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.process_incoming();
        apply_custom_theme(ctx, &self.settings_theme);

        // ── Model download startup panel ───────────────────────────
        let current_status = self.state.model_status.lock().unwrap().clone();
        match &current_status {
            ModelStatus::Ready => {
                if !self.model_ready {
                    self.model_ready = true;
                }
            }
            ModelStatus::Failed(_) => {
                self.show_startup_panel(ctx, &current_status);
                ctx.request_repaint();
                return;
            }
            _ => {
                self.show_startup_panel(ctx, &current_status);
                ctx.request_repaint();
                return;
            }
        }

        // ── Track window geometry ──────────────────────────────────
        let screen = ctx.input(|i| i.screen_rect);
        self.window_x = screen.left();
        self.window_y = screen.top();
        self.window_w = screen.width();
        self.window_h = screen.height();

        // ═══════════════════════════════════════════════════════════
        // STATUS BAR (top)
        // ═══════════════════════════════════════════════════════════
        egui::TopBottomPanel::top("status_bar")
            .min_height(36.0)
            .frame(egui::Frame {
                fill: panel_fill(&self.settings_theme),
                inner_margin: egui::Margin::symmetric(16, 6),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Recording indicator
                    let (dot_color, label) = if self.current_snapshot.is_recording {
                        (if is_dark(&self.settings_theme) { color::dark::RECORDING } else { color::light::RECORDING },
                         self.ui_strings.status_recording())
                    } else {
                        (text_muted(&self.settings_theme), self.ui_strings.status_idle())
                    };
                    let dot = egui::epaint::CircleShape {
                        center: egui::pos2(0.0, 0.0),
                        radius: 4.0,
                        fill: dot_color,
                        stroke: egui::Stroke::NONE,
                    };
                    ui.painter().circle(dot.center, dot.radius, dot.fill, dot.stroke);
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(label)
                            .size(13.0)
                            .color(text_primary(&self.settings_theme)),
                    );

                    ui.separator();

                    // Language + conversion badges
                    ui.label(
                        egui::RichText::new(self.ui_strings.lang_label(&self.current_snapshot.current_language))
                            .size(12.0)
                            .color(text_secondary(&self.settings_theme)),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(self.ui_strings.convert_label(&self.current_snapshot.conversion_mode))
                            .size(12.0)
                            .color(text_secondary(&self.settings_theme)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Settings button
                        let settings_btn = egui::Button::new(
                            egui::RichText::new(self.ui_strings.settings_label())
                                .size(12.0),
                        )
                        .fill(if self.show_settings {
                            accent_color(&self.settings_theme)
                        } else {
                            egui::Color32::TRANSPARENT
                        });
                        if ui.add(settings_btn).clicked() {
                            self.show_settings = !self.show_settings;
                        }

                        // Overlay toggle
                        let ov_label = if self.show_overlay_local {
                            self.ui_strings.hide_overlay()
                        } else {
                            self.ui_strings.show_overlay()
                        };
                        let ov_btn = egui::Button::new(
                            egui::RichText::new(ov_label).size(12.0),
                        )
                        .fill(egui::Color32::TRANSPARENT);
                        if ui.add(ov_btn).clicked() {
                            self.show_overlay_local = !self.show_overlay_local;
                            self.state.show_overlay.store(self.show_overlay_local, Ordering::SeqCst);
                            self.send_action(GuiAction::ShowOverlay(self.show_overlay_local));
                        }
                    });
                });
            });

        // ═══════════════════════════════════════════════════════════
        // CENTRAL: Transcript + History
        // ═══════════════════════════════════════════════════════════
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: if is_dark(&self.settings_theme) { color::dark::BG } else { color::light::BG },
                inner_margin: egui::Margin::symmetric(16, 12),
                ..Default::default()
            })
            .show(ctx, |ui| {
                // ── Live Transcript Card ────────────────────────────
                        egui::Frame::NONE
                            .fill(card_fill(&self.settings_theme))
                            .stroke(egui::Stroke::new(1.0, border_color(&self.settings_theme)))
                            .corner_radius(8)
                            .inner_margin(egui::epaint::Marginf::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(
                                egui::RichText::new(self.ui_strings.live_transcript())
                                    .size(16.0)
                                    .color(text_primary(&self.settings_theme)),
                            );
                            if self.current_snapshot.is_recording {
                                let rec_dot_color = if is_dark(&self.settings_theme) { color::dark::RECORDING } else { color::light::RECORDING };
                                ui.painter().circle(egui::pos2(0.0, 0.0), 3.0, rec_dot_color, egui::Stroke::NONE);
                            }
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Final text
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(self.ui_strings.final_prefix())
                                    .size(12.0)
                                    .color(text_secondary(&self.settings_theme))
                                    .strong(),
                            );
                            ui.colored_label(
                                text_primary(&self.settings_theme),
                                &self.current_snapshot.latest_final_text,
                            );
                        });
                        ui.add_space(4.0);

                        // Partial text
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(self.ui_strings.partial_prefix())
                                    .size(12.0)
                                    .color(text_muted(&self.settings_theme)),
                            );
                            ui.colored_label(
                                text_secondary(&self.settings_theme),
                                &self.current_snapshot.latest_partial_text,
                            );
                        });
                    });

                ui.add_space(16.0);

                // ── History Section ─────────────────────────────────
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new(self.ui_strings.history_label())
                            .size(16.0)
                            .color(text_primary(&self.settings_theme)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(
                            egui::RichText::new(self.ui_strings.clear_all())
                                .size(12.0)
                                .color(accent_color(&self.settings_theme)),
                        ).clicked() {
                            self.send_action(GuiAction::ClearHistory);
                            self.current_snapshot.history.clear();
                        }
                    });
                });
                ui.add_space(8.0);

                // History list
                let available = ui.available_height() - 60.0;
                egui::ScrollArea::vertical()
                    .max_height(available.max(80.0))
                    .show(ui, |ui| {
                        if self.current_snapshot.history.is_empty() {
                            ui.add_space(24.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("—")
                                        .size(12.0)
                                        .color(text_muted(&self.settings_theme)),
                                );
                            });
                            return;
                        }

                        let mut to_delete: Option<usize> = None;
                        for (idx, entry) in self.current_snapshot.history.iter().enumerate() {
                            let row_color = if idx % 2 == 0 {
                                egui::Color32::TRANSPARENT
                            } else {
                                if is_dark(&self.settings_theme) {
                                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 4)
                                } else {
                                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 2)
                                }
                            };

                            egui::Frame::NONE
                                .fill(row_color)
                                .corner_radius(4)
                                .inner_margin(egui::epaint::Marginf::symmetric(8.0, 6.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.set_height(28.0);
                                        // Timestamp
                                        ui.label(
                                            egui::RichText::new(&entry.timestamp)
                                                .size(11.0)
                                                .color(text_muted(&self.settings_theme))
                                                .monospace(),
                                        );
                                        ui.add_space(8.0);
                                        // Text
                                        ui.add(egui::Label::new(
                                            egui::RichText::new(&entry.text)
                                                .size(13.0)
                                                .color(text_primary(&self.settings_theme)),
                                        ).wrap());
                                        // Actions
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button(
                                                egui::RichText::new(self.ui_strings.delete_label())
                                                    .size(11.0)
                                                    .color(if is_dark(&self.settings_theme) { color::dark::ERROR } else { color::light::ERROR }),
                                            ).clicked() {
                                                to_delete = Some(idx);
                                            }
                                            if ui.button(
                                                egui::RichText::new(self.ui_strings.copy_label())
                                                    .size(11.0),
                                            ).clicked() {
                                                ui.ctx().copy_text(entry.text.clone());
                                            }
                                        });
                                    });
                                });
                            ui.add_space(2.0);
                        }

                        if let Some(idx) = to_delete {
                            self.send_action(GuiAction::DeleteHistoryEntry(idx));
                            if idx < self.current_snapshot.history.len() {
                                self.current_snapshot.history.remove(idx);
                            }
                        }
                    });
            });

        // ═══════════════════════════════════════════════════════════
        // BOTTOM CONTROLS
        // ═══════════════════════════════════════════════════════════
        egui::TopBottomPanel::bottom("controls")
            .min_height(52.0)
            .frame(egui::Frame {
                fill: panel_fill(&self.settings_theme),
                inner_margin: egui::Margin::symmetric(16, 8),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Recording button (primary action)
                    let is_rec = self.current_snapshot.is_recording;
                    let btn_label = if is_rec {
                        self.ui_strings.stop_recording_label()
                    } else {
                        self.ui_strings.start_recording_label()
                    };
                    let btn_color = if is_rec {
                        if is_dark(&self.settings_theme) { color::dark::RECORDING } else { color::light::RECORDING }
                    } else {
                        accent_color(&self.settings_theme)
                    };
                    let rec_btn = egui::Button::new(
                        egui::RichText::new(btn_label)
                            .size(14.0)
                            .color(text_primary(&self.settings_theme)),
                    )
                    .min_size(egui::vec2(130.0, 34.0))
                    .fill(btn_color);
                    if ui.add(rec_btn).clicked() {
                        self.send_action(GuiAction::ToggleRecording);
                    }

                    ui.add_space(8.0);

                    // Cycle Lang
                    if ui.add(
                        egui::Button::new(
                            egui::RichText::new(self.ui_strings.cycle_language_label())
                                .size(13.0),
                        )
                        .min_size(egui::vec2(100.0, 34.0))
                    ).clicked() {
                        self.send_action(GuiAction::CycleLanguage);
                    }

                    // Flush
                    if ui.add(
                        egui::Button::new(
                            egui::RichText::new(self.ui_strings.flush_label())
                                .size(13.0),
                        )
                        .min_size(egui::vec2(90.0, 34.0))
                    ).clicked() {
                        self.send_action(GuiAction::Flush);
                    }
                });
            });

        // ═══════════════════════════════════════════════════════════
        // SETTINGS WINDOW (modal overlay)
        // ═══════════════════════════════════════════════════════════
        if self.show_settings {
            let mut pending_save: Option<crate::config::AppConfig> = None;
            let _dark = is_dark(&self.settings_theme);

            egui::Window::new(self.ui_strings.settings_title())
                .default_size([440.0, 520.0])
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // ── General Section ─────────────────────────
                        egui::CollapsingHeader::new(
                            egui::RichText::new(self.ui_strings.settings_general_section())
                                .size(14.0)
                                .color(text_primary(&self.settings_theme))
                                .strong(),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            egui::Grid::new("general_grid")
                                .striped(false)
                                .min_col_width(140.0)
                                .spacing(egui::vec2(12.0, 6.0))
                                .show(ui, |ui| {
                                    // UI Language
                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_ui_language())
                                            .size(13.0),
                                    );
                                    egui::ComboBox::from_id_salt("ui_lang")
                                        .selected_text(
                                            UiLang::from_code(&self.settings_ui_lang).display_name()
                                        )
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            let langs = [
                                                (UiLang::ChineseTraditional, "zh-TW"),
                                                (UiLang::ChineseSimplified, "zh-CN"),
                                                (UiLang::English, "en"),
                                            ];
                                            for (variant, code) in &langs {
                                                let name = variant.display_name();
                                                if ui.selectable_label(
                                                    self.settings_ui_lang == *code,
                                                    name,
                                                ).clicked() {
                                                    self.settings_ui_lang = code.to_string();
                                                }
                                            }
                                        });
                                    ui.end_row();

                                    // Theme
                                    ui.label(
                                        egui::RichText::new(self.ui_strings.theme_label())
                                            .size(13.0),
                                    );
                                    egui::ComboBox::from_id_salt("theme")
                                        .selected_text(&self.settings_theme)
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.settings_theme, "Dark".to_owned(), "Dark");
                                            ui.selectable_value(&mut self.settings_theme, "Light".to_owned(), "Light");
                                        });
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                        });

                        ui.add_space(4.0);

                        // ── ASR Section ─────────────────────────────
                        egui::CollapsingHeader::new(
                            egui::RichText::new(self.ui_strings.settings_asr_section())
                                .size(14.0)
                                .color(text_primary(&self.settings_theme))
                                .strong(),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            egui::Grid::new("asr_grid")
                                .striped(false)
                                .min_col_width(140.0)
                                .spacing(egui::vec2(12.0, 6.0))
                                .show(ui, |ui| {
                                    let langs = ["auto", "zh", "en", "ja", "de", "fr", "es", "ko"];

                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_asr_language())
                                            .size(13.0),
                                    );
                                    egui::ComboBox::from_id_salt("asr_lang")
                                        .selected_text(&self.settings_language)
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            for lang in &langs {
                                                ui.selectable_value(&mut self.settings_language, lang.to_string(), *lang);
                                            }
                                        });
                                    ui.end_row();

                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_provider())
                                            .size(13.0),
                                    );
                                    egui::ComboBox::from_id_salt("provider")
                                        .selected_text(&self.settings_provider)
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.settings_provider, "cpu".to_owned(), "cpu");
                                            ui.selectable_value(&mut self.settings_provider, "cuda".to_owned(), "cuda");
                                        });
                                    ui.end_row();

                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_threads())
                                            .size(13.0),
                                    );
                                    ui.add(egui::DragValue::new(&mut self.settings_num_threads).range(1..=16).speed(0.5));
                                    ui.end_row();

                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_decoding())
                                            .size(13.0),
                                    );
                                    egui::ComboBox::from_id_salt("decoding")
                                        .selected_text(&self.settings_decoding_method)
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.settings_decoding_method, "greedy_search".to_owned(), "greedy_search");
                                            ui.selectable_value(&mut self.settings_decoding_method, "modified_beam_search".to_owned(), "modified_beam_search");
                                        });
                                    ui.end_row();

                                    // VAD
                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_vad())
                                            .size(13.0),
                                    );
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut self.settings_use_vad, "");
                                        ui.add_space(4.0);
                                        ui.label(
                                            egui::RichText::new(self.ui_strings.settings_enabled())
                                                .size(12.0),
                                        );
                                    });
                                    ui.end_row();

                                    if self.settings_use_vad {
                                        ui.label(
                                            egui::RichText::new(self.ui_strings.settings_vad_threshold())
                                                .size(13.0),
                                        );
                                        ui.add(egui::Slider::new(&mut self.settings_vad_threshold, 0.01..=0.99)
                                            .step_by(0.01)
                                            .show_value(true));
                                        ui.end_row();
                                    }
                                });
                            ui.add_space(4.0);
                        });

                        ui.add_space(4.0);

                        // ── Injection Section ──────────────────────
                        egui::CollapsingHeader::new(
                            egui::RichText::new(self.ui_strings.settings_injection_section())
                                .size(14.0)
                                .color(text_primary(&self.settings_theme))
                                .strong(),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            egui::Grid::new("inject_grid")
                                .striped(false)
                                .min_col_width(140.0)
                                .spacing(egui::vec2(12.0, 6.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_inject_strategy())
                                            .size(13.0),
                                    );
                                    egui::ComboBox::from_id_salt("inject")
                                        .selected_text(&self.settings_inject_strategy)
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.settings_inject_strategy, "auto".to_owned(), "auto");
                                            ui.selectable_value(&mut self.settings_inject_strategy, "clipboard".to_owned(), "clipboard");
                                            ui.selectable_value(&mut self.settings_inject_strategy, "sendinput".to_owned(), "sendinput");
                                        });
                                    ui.end_row();

                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_key_delay())
                                            .size(13.0),
                                    );
                                    ui.add(egui::DragValue::new(&mut self.settings_key_delay_ms).range(0..=100).speed(1.0));
                                    ui.end_row();

                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_restore_clipboard())
                                            .size(13.0),
                                    );
                                    ui.checkbox(&mut self.settings_restore_clipboard, "");
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                        });

                        ui.add_space(4.0);

                        // ── Conversion Section ─────────────────────
                        egui::CollapsingHeader::new(
                            egui::RichText::new(self.ui_strings.settings_conversion_section())
                                .size(14.0)
                                .color(text_primary(&self.settings_theme))
                                .strong(),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            egui::Grid::new("conv_grid")
                                .striped(false)
                                .min_col_width(140.0)
                                .spacing(egui::vec2(12.0, 6.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(self.ui_strings.settings_conversion_mode())
                                            .size(13.0),
                                    );
                                    let modes = ["none", "s2t", "t2s"];
                                    egui::ComboBox::from_id_salt("conversion")
                                        .selected_text(&self.settings_conversion_mode)
                                        .width(160.0)
                                        .show_ui(ui, |ui| {
                                            for mode in &modes {
                                                ui.selectable_value(&mut self.settings_conversion_mode, mode.to_string(), *mode);
                                            }
                                        });
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                        });

                        ui.add_space(4.0);

                        // ── Hotkeys Section ─────────────────────────
                        egui::CollapsingHeader::new(
                            egui::RichText::new(self.ui_strings.settings_hotkeys_section())
                                .size(14.0)
                                .color(text_primary(&self.settings_theme))
                                .strong(),
                        )
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(self.ui_strings.hotkey_display())
                                    .size(12.0)
                                    .color(text_secondary(&self.settings_theme)),
                            );
                            ui.add_space(4.0);
                            for (action, key) in [
                                (self.ui_strings.hotkey_toggle_label(), "Ctrl+Shift+F2"),
                                (self.ui_strings.hotkey_lang_label(), "Ctrl+Shift+L"),
                                (self.ui_strings.hotkey_flush_label(), "Ctrl+Shift+Space"),
                                (self.ui_strings.hotkey_ptt_label(), "Ctrl+Shift+F2 (hold)"),
                            ] {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("  • {}", action))
                                            .size(12.0)
                                            .color(text_primary(&self.settings_theme)),
                                    );
                                    ui.label(
                                        egui::RichText::new(key)
                                            .size(12.0)
                                            .color(accent_color(&self.settings_theme))
                                            .monospace(),
                                    );
                                });
                            }
                            ui.add_space(4.0);
                        });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(12.0);

                        // ── Save / Cancel ──────────────────────────
                        ui.horizontal(|ui| {
                            if ui.add(
                                egui::Button::new(
                                    egui::RichText::new(self.ui_strings.settings_save())
                                        .size(14.0),
                                )
                                .min_size(egui::vec2(120.0, 34.0))
                                .fill(accent_color(&self.settings_theme))
                            ).clicked() {
                                let mut cfg = self.base_config.clone().unwrap_or_default();
                                cfg.language.language = self.settings_language.clone();
                                cfg.asr.provider = self.settings_provider.clone();
                                cfg.asr.num_threads = self.settings_num_threads;
                                cfg.asr.use_vad = self.settings_use_vad;
                                cfg.asr.vad_threshold = self.settings_vad_threshold;
                                cfg.asr.decoding_method = self.settings_decoding_method.clone();
                                cfg.injector.strategy = self.settings_inject_strategy.clone();
                                cfg.injector.key_delay_ms = self.settings_key_delay_ms;
                                cfg.injector.restore_clipboard = self.settings_restore_clipboard;
                                cfg.conversion.mode = self.settings_conversion_mode.clone();
                                cfg.ui.language = self.settings_ui_lang.clone();
                                cfg.ui.theme = self.settings_theme.clone();
                                cfg.ui.window_x = Some(self.window_x);
                                cfg.ui.window_y = Some(self.window_y);
                                cfg.ui.window_width = Some(self.window_w);
                                cfg.ui.window_height = Some(self.window_h);
                                self.base_config = Some(cfg.clone());
                                pending_save = Some(cfg);
                                self.show_settings = false;
                            }

                            ui.add_space(8.0);

                            if ui.add(
                                egui::Button::new(
                                    egui::RichText::new(self.ui_strings.settings_cancel())
                                        .size(14.0),
                                )
                                .min_size(egui::vec2(100.0, 34.0))
                            ).clicked() {
                                self.show_settings = false;
                            }
                        });
                    });
                });

            if let Some(cfg) = pending_save {
                self.send_action(GuiAction::SaveConfig(cfg));
            }
        }

        // ═══════════════════════════════════════════════════════════
        // OVERLAY
        // ═══════════════════════════════════════════════════════════
        let ov_text = if self.current_snapshot.latest_final_text.is_empty() {
            &self.current_snapshot.latest_partial_text
        } else {
            &self.current_snapshot.latest_final_text
        };
        crate::ui::overlay::show_overlay_viewport(ctx, ov_text, self.show_overlay_local);

        // Repaint while recording for smooth transcript updates
        if self.current_snapshot.is_recording {
            ctx.request_repaint();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Entry Point
// ═══════════════════════════════════════════════════════════════════

/// Run the eframe GUI on the calling thread (blocks until window closes).
pub fn run_gui(
    snapshot: Arc<Mutex<GuiSnapshot>>,
    gui_rx: crossbeam::channel::Receiver<GuiSnapshot>,
    action_tx: crossbeam::channel::Sender<GuiAction>,
    show_overlay: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    initial_pos: Option<egui::Pos2>,
    initial_size: Option<egui::Vec2>,
    initial_theme: Option<String>,
    initial_lang: UiLang,
    initial_config: Option<&AppConfig>,
    model_status: Arc<Mutex<ModelStatus>>,
) {
    let shared_state = GuiSharedState {
        snapshot,
        gui_rx,
        action_tx,
        show_overlay,
        is_running,
        model_status,
    };
    let title_strings = Strings::new(initial_lang);

    let mut vp = egui::ViewportBuilder::default()
        .with_min_inner_size([480.0, 360.0])
        .with_title(title_strings.app_name());
    if let Some(pos) = initial_pos {
        vp = vp.with_position(pos);
    }
    if let Some(size) = initial_size {
        vp = vp.with_inner_size(size);
    } else {
        vp = vp.with_inner_size([800.0, 600.0]);
    }

    let options = eframe::NativeOptions {
        viewport: vp,
        ..Default::default()
    };

    tracing::info!("Launching egui window...");
    match eframe::run_native(
        "Nemotron Voice Input",
        options,
        Box::new(|cc| {
            setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(GuiApp::new(
                shared_state,
                initial_pos,
                initial_size,
                initial_theme,
                initial_lang,
                initial_config,
            )))
        }),
    ) {
        Ok(()) => tracing::info!("egui window closed"),
        Err(e) => tracing::error!("egui fatal error: {}", e),
    }
}
