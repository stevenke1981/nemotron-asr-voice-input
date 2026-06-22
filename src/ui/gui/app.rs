use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use super::state::{GuiAction, GuiSnapshot};
use crate::ui::strings::{Strings, UiLang};

/// List of common Chinese font paths on Windows, in priority order.
const CHINESE_FONT_PATHS: &[&str] = &[
    "C:\\Windows\\Fonts\\msyh.ttc",   // Microsoft YaHei (Simplified)
    "C:\\Windows\\Fonts\\msjh.ttc",   // Microsoft JhengHei (Traditional)
    "C:\\Windows\\Fonts\\simsun.ttc", // SimSun (Song)
    "C:\\Windows\\Fonts\\simhei.ttf", // SimHei
    "C:\\Windows\\Fonts\\deng.ttf",   // DengXian
];

/// Set up egui fonts to support CJK characters by loading a system Chinese font.
/// Falls back to default fonts if no Chinese font is found.
fn setup_chinese_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in CHINESE_FONT_PATHS {
        match std::fs::read(path) {
            Ok(data) => {
                let font_name = format!("chinese_{}", std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("font"));
                fonts.font_data.insert(font_name.clone(), Arc::new(egui::FontData::from_owned(data)));
                // Insert at index 0 so CJK glyphs take priority over the default Latin font
                fonts.families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, font_name);
                tracing::info!("Loaded Chinese font: {}", path);
                break;
            }
            Err(_) => continue,
        }
    }
    ctx.set_fonts(fonts);
}

pub struct GuiSharedState {
    pub snapshot: Arc<Mutex<GuiSnapshot>>,
    pub gui_rx: crossbeam::channel::Receiver<GuiSnapshot>,
    pub action_tx: crossbeam::channel::Sender<GuiAction>,
    pub show_overlay: Arc<AtomicBool>,
    /// Set to false to signal the GUI to close (e.g., tray Exit).
    pub is_running: Arc<AtomicBool>,
}

pub struct GuiApp {
    state: GuiSharedState,
    current_snapshot: GuiSnapshot,
    show_settings: bool,
    show_overlay_local: bool,
    // Settings state
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
    // Window geometry tracking
    window_x: f32,
    window_y: f32,
    window_w: f32,
    window_h: f32,
    /// Bilingual UI strings.
    ui_strings: Strings,
}

impl GuiApp {
    pub fn new(state: GuiSharedState, initial_pos: Option<egui::Pos2>, initial_size: Option<egui::Vec2>, initial_theme: Option<String>, initial_lang: UiLang) -> Self {
        let (wx, wy) = initial_pos.map(|p| (p.x, p.y)).unwrap_or((100.0, 100.0));
        let (ww, wh) = initial_size.map(|s| (s.x, s.y)).unwrap_or((800.0, 600.0));
        let theme = initial_theme.unwrap_or_else(|| "Dark".into());
        Self {
            state,
            current_snapshot: GuiSnapshot::default(),
            show_settings: false,
            show_overlay_local: false,
            settings_language: "zh".into(),
            settings_provider: "cpu".into(),
            settings_num_threads: 4,
            settings_use_vad: true,
            settings_vad_threshold: 0.1,
            settings_decoding_method: "greedy_search".into(),
            settings_inject_strategy: "auto".into(),
            settings_key_delay_ms: 5,
            settings_restore_clipboard: true,
            settings_conversion_mode: "s2t".into(),
            settings_ui_lang: match initial_lang {
                UiLang::English => "en",
                UiLang::Chinese => "zh",
            }.to_string(),
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
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Exit if the application is shutting down (e.g., tray Exit was clicked)
        if !self.state.is_running.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.process_incoming();

        // Apply theme
        if self.settings_theme == "Light" {
            ctx.set_visuals(egui::Visuals::light());
        } else {
            ctx.set_visuals(egui::Visuals::dark());
        }

        // Track window geometry from egui input state
        let screen = ctx.input(|i| i.screen_rect);
        self.window_x = screen.left();
        self.window_y = screen.top();
        self.window_w = screen.width();
        self.window_h = screen.height();

        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (icon, status) = if self.current_snapshot.is_recording {
                    ("REC", self.ui_strings.status_recording())
                } else {
                    ("--", self.ui_strings.status_idle())
                };
                ui.label(format!("{} {}", icon, status));
                ui.separator();
                ui.label(self.ui_strings.lang_label(&self.current_snapshot.current_language));
                ui.separator();
                ui.label(self.ui_strings.convert_label(&self.current_snapshot.conversion_mode));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(self.ui_strings.settings_label()).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    let ov_label = if self.show_overlay_local {
                        self.ui_strings.hide_overlay()
                    } else {
                        self.ui_strings.show_overlay()
                    };
                    if ui.button(ov_label).clicked() {
                        self.show_overlay_local = !self.show_overlay_local;
                        self.state
                            .show_overlay
                            .store(self.show_overlay_local, Ordering::SeqCst);
                        self.send_action(GuiAction::ShowOverlay(self.show_overlay_local));
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.ui_strings.live_transcript());
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(self.ui_strings.final_prefix());
                ui.colored_label(egui::Color32::WHITE, &self.current_snapshot.latest_final_text);
            });
            ui.horizontal(|ui| {
                ui.label(self.ui_strings.partial_prefix());
                ui.colored_label(egui::Color32::GRAY, &self.current_snapshot.latest_partial_text);
            });
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.heading(self.ui_strings.history_label());
                if ui.button(self.ui_strings.clear_all()).clicked() {
                    self.send_action(GuiAction::ClearHistory);
                    self.current_snapshot.history.clear();
                }
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 40.0)
                .show(ui, |ui| {
                    let mut to_delete: Option<usize> = None;
                    for (idx, entry) in self.current_snapshot.history.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(&entry.timestamp);
                            ui.label(&entry.text);
                            if ui.button(self.ui_strings.copy_label()).clicked() {
                                ui.ctx().copy_text(entry.text.clone());
                            }
                            if ui.button(self.ui_strings.delete_label()).clicked() {
                                to_delete = Some(idx);
                            }
                        });
                        ui.separator();
                    }
                    if let Some(idx) = to_delete {
                        self.send_action(GuiAction::DeleteHistoryEntry(idx));
                        if idx < self.current_snapshot.history.len() {
                            self.current_snapshot.history.remove(idx);
                        }
                    }
                });
        });

        egui::TopBottomPanel::bottom("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let btn_label = if self.current_snapshot.is_recording {
                    self.ui_strings.stop_recording_label()
                } else {
                    self.ui_strings.start_recording_label()
                };
                if ui.button(btn_label).clicked() {
                    self.send_action(GuiAction::ToggleRecording);
                }
                if ui.button(self.ui_strings.cycle_language_label()).clicked() {
                    self.send_action(GuiAction::CycleLanguage);
                }
                if ui.button(self.ui_strings.flush_label()).clicked() {
                    self.send_action(GuiAction::Flush);
                }
            });
        });

        if self.show_settings {
            let mut pending_save: Option<crate::config::AppConfig> = None;
            egui::Window::new(self.ui_strings.settings_title())
                .default_size([400.0, 500.0])
                .show(ctx, |ui| {
                    egui::Grid::new("settings_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(self.ui_strings.settings_ui_language());
                            egui::ComboBox::from_id_salt("ui_lang")
                                .selected_text(self.ui_strings.settings_ui_language())
                                .show_ui(ui, |ui| {
                                    let en_name = if self.ui_strings.lang == UiLang::Chinese { "英文" } else { "English" };
                                    let zh_name = if self.ui_strings.lang == UiLang::Chinese { "中文" } else { "Chinese" };
                                    if ui.selectable_label(false, en_name).clicked() {
                                        self.settings_ui_lang = "en".to_owned();
                                    }
                                    if ui.selectable_label(false, zh_name).clicked() {
                                        self.settings_ui_lang = "zh".to_owned();
                                    }
                                });
                            ui.end_row();

                            ui.label(self.ui_strings.settings_asr_language());
                            let langs = ["auto", "zh", "en", "ja", "de", "fr", "es", "ko"];
                            egui::ComboBox::from_id_salt("asr_lang")
                                .selected_text(&self.settings_language)
                                .show_ui(ui, |ui| {
                                    for lang in &langs {
                                        ui.selectable_value(&mut self.settings_language, lang.to_string(), *lang);
                                    }
                                });
                            ui.end_row();

                            ui.label(self.ui_strings.settings_provider());
                            egui::ComboBox::from_id_salt("provider")
                                .selected_text(&self.settings_provider)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_provider, "cpu".to_owned(), "cpu");
                                    ui.selectable_value(&mut self.settings_provider, "cuda".to_owned(), "cuda");
                                });
                            ui.end_row();

                            ui.label(self.ui_strings.settings_threads());
                            ui.add(egui::DragValue::new(&mut self.settings_num_threads).range(1..=16));
                            ui.end_row();

                            ui.label(self.ui_strings.settings_vad());
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.settings_use_vad, self.ui_strings.settings_enabled());
                                ui.add_space(8.0);
                                ui.label(self.ui_strings.settings_vad_threshold());
                                ui.add(egui::Slider::new(&mut self.settings_vad_threshold, 0.01..=0.99).step_by(0.01));
                            });
                            ui.end_row();

                            ui.label(self.ui_strings.settings_decoding());
                            egui::ComboBox::from_id_salt("decoding")
                                .selected_text(&self.settings_decoding_method)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_decoding_method, "greedy_search".to_owned(), "greedy_search");
                                    ui.selectable_value(&mut self.settings_decoding_method, "modified_beam_search".to_owned(), "modified_beam_search");
                                });
                            ui.end_row();

                            ui.label(self.ui_strings.settings_inject_strategy());
                            egui::ComboBox::from_id_salt("inject")
                                .selected_text(&self.settings_inject_strategy)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_inject_strategy, "sendinput".to_owned(), "sendinput");
                                    ui.selectable_value(&mut self.settings_inject_strategy, "clipboard".to_owned(), "clipboard");
                                    ui.selectable_value(&mut self.settings_inject_strategy, "auto".to_owned(), "auto");
                                });
                            ui.end_row();

                            ui.label(self.ui_strings.settings_key_delay());
                            ui.add(egui::DragValue::new(&mut self.settings_key_delay_ms).range(0..=100));
                            ui.end_row();

                            ui.label(self.ui_strings.settings_restore_clipboard());
                            ui.checkbox(&mut self.settings_restore_clipboard, self.ui_strings.settings_yes());
                            ui.end_row();

                            ui.label(self.ui_strings.settings_conversion_mode());
                            let modes = ["none", "s2t", "t2s"];
                            egui::ComboBox::from_id_salt("conversion")
                                .selected_text(&self.settings_conversion_mode)
                                .show_ui(ui, |ui| {
                                    for mode in &modes {
                                        ui.selectable_value(&mut self.settings_conversion_mode, mode.to_string(), *mode);
                                    }
                                });
                            ui.end_row();

                            ui.label(self.ui_strings.theme_label());
                            egui::ComboBox::from_id_salt("theme")
                                .selected_text(&self.settings_theme)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_theme, "Dark".to_owned(), "Dark");
                                    ui.selectable_value(&mut self.settings_theme, "Light".to_owned(), "Light");
                                });
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(self.ui_strings.hotkey_display());
                    ui.label(self.ui_strings.settings_hotkey_line(self.ui_strings.hotkey_toggle_label(), "Ctrl+Shift+F2"));
                    ui.label(self.ui_strings.settings_hotkey_line(self.ui_strings.hotkey_lang_label(), "Ctrl+Shift+L"));
                    ui.label(self.ui_strings.settings_hotkey_line(self.ui_strings.hotkey_flush_label(), "Ctrl+Shift+Space"));

                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button(self.ui_strings.settings_save()).clicked() {
                            let mut cfg = crate::config::AppConfig::default();
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
                            pending_save = Some(cfg);
                            self.show_settings = false;
                        }
                        if ui.button(self.ui_strings.settings_cancel()).clicked() {
                            self.show_settings = false;
                        }
                    });
                });
            if let Some(cfg) = pending_save {
                self.send_action(GuiAction::SaveConfig(cfg));
            }
        }

        // Overlay viewport
        let ov_text = if self.current_snapshot.latest_final_text.is_empty() {
            &self.current_snapshot.latest_partial_text
        } else {
            &self.current_snapshot.latest_final_text
        };
        crate::ui::overlay::show_overlay_viewport(ctx, ov_text, self.show_overlay_local);

        if self.current_snapshot.is_recording {
            ctx.request_repaint();
        }
    }
}

/// Run the eframe GUI on the calling thread (blocks until window closes).
///
/// NOTE: On Windows, winit requires the event loop on the main thread,
/// so call this from the main thread and run the Win32 message loop
/// on a background thread.
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
) {
    let shared_state = GuiSharedState {
        snapshot,
        gui_rx,
        action_tx,
        show_overlay,
        is_running,
    };
    let title_strings = Strings::new(initial_lang);
    let window_title = title_strings.app_name().to_owned();
    let mut vp = egui::ViewportBuilder::default()
        .with_min_inner_size([400.0, 300.0])
        .with_title(&window_title);
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
    tracing::info!("Calling eframe::run_native on main thread...");
    match eframe::run_native(
        "Nemotron Voice Input",
        options,
        Box::new(|cc| {
            setup_chinese_fonts(&cc.egui_ctx);
            Ok(Box::new(GuiApp::new(shared_state, initial_pos, initial_size, initial_theme, initial_lang)))
        }),
    ) {
        Ok(()) => tracing::info!("eframe run_native returned Ok"),
        Err(e) => {
            tracing::error!("eframe GUI exited with error: {}", e);
        }
    }
}
