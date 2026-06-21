use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::state::{GuiAction, GuiSnapshot};

pub struct GuiSharedState {
    pub snapshot: Arc<Mutex<GuiSnapshot>>,
    pub gui_rx: crossbeam::channel::Receiver<GuiSnapshot>,
    pub action_tx: crossbeam::channel::Sender<GuiAction>,
    pub show_overlay: Arc<AtomicBool>,
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
    settings_decoding_method: String,
    settings_inject_strategy: String,
    settings_key_delay_ms: u64,
    settings_restore_clipboard: bool,
    settings_conversion_mode: String,
    settings_ui_lang: String,
    // Window geometry tracking
    window_x: f32,
    window_y: f32,
    window_w: f32,
    window_h: f32,
}

impl GuiApp {
    pub fn new(state: GuiSharedState, initial_pos: Option<egui::Pos2>, initial_size: Option<egui::Vec2>) -> Self {
        let (wx, wy) = initial_pos.map(|p| (p.x, p.y)).unwrap_or((100.0, 100.0));
        let (ww, wh) = initial_size.map(|s| (s.x, s.y)).unwrap_or((800.0, 600.0));
        Self {
            state,
            current_snapshot: GuiSnapshot::default(),
            show_settings: false,
            show_overlay_local: false,
            settings_language: "zh".into(),
            settings_provider: "cpu".into(),
            settings_num_threads: 4,
            settings_use_vad: true,
            settings_decoding_method: "greedy_search".into(),
            settings_inject_strategy: "auto".into(),
            settings_key_delay_ms: 5,
            settings_restore_clipboard: true,
            settings_conversion_mode: "s2t".into(),
            settings_ui_lang: "English".into(),
            window_x: wx,
            window_y: wy,
            window_w: ww,
            window_h: wh,
        }
    }

    fn process_incoming(&mut self) {
        while let Ok(snapshot) = self.state.gui_rx.try_recv() {
            self.current_snapshot = snapshot;
        }
    }

    fn send_action(&self, action: GuiAction) {
        let _ = self.state.action_tx.send(action);
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_incoming();

        // Track window geometry from egui input state
        let screen = ctx.input(|i| i.screen_rect);
        self.window_x = screen.left();
        self.window_y = screen.top();
        self.window_w = screen.width();
        self.window_h = screen.height();

        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (icon, label) = if self.current_snapshot.is_recording {
                    ("REC", "Recording")
                } else {
                    ("--", "Idle")
                };
                ui.label(format!("{} {}", icon, label));
                ui.separator();
                ui.label(format!("Lang: {}", self.current_snapshot.current_language));
                ui.separator();
                ui.label(format!("Convert: {}", self.current_snapshot.conversion_mode));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Settings").clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    let ov_label = if self.show_overlay_local {
                        "Hide Overlay"
                    } else {
                        "Show Overlay"
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
            ui.heading("Live Transcript");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Final:");
                ui.colored_label(egui::Color32::WHITE, &self.current_snapshot.latest_final_text);
            });
            ui.horizontal(|ui| {
                ui.label("Partial:");
                ui.colored_label(egui::Color32::GRAY, &self.current_snapshot.latest_partial_text);
            });
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.heading("History");
                if ui.button("Clear All").clicked() {
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
                            if ui.button("Copy").clicked() {
                                ui.ctx().copy_text(entry.text.clone());
                            }
                            if ui.button("Del").clicked() {
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
                    "Stop Recording"
                } else {
                    "Start Recording"
                };
                if ui.button(btn_label).clicked() {
                    self.send_action(GuiAction::ToggleRecording);
                }
                if ui.button("Cycle Language").clicked() {
                    self.send_action(GuiAction::CycleLanguage);
                }
                if ui.button("Flush").clicked() {
                    self.send_action(GuiAction::Flush);
                }
            });
        });

        if self.show_settings {
            let mut pending_save: Option<crate::config::AppConfig> = None;
            egui::Window::new("Settings")
                .default_size([400.0, 500.0])
                .show(ctx, |ui| {
                    egui::Grid::new("settings_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("UI Language:");
                            egui::ComboBox::from_id_salt("ui_lang")
                                .selected_text(&self.settings_ui_lang)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_ui_lang, "English".to_owned(), "English");
                                    ui.selectable_value(&mut self.settings_ui_lang, "Chinese".to_owned(), "Chinese");
                                });
                            ui.end_row();

                            ui.label("ASR Language:");
                            let langs = ["zh", "en", "ja", "de", "fr", "es", "ko"];
                            egui::ComboBox::from_id_salt("asr_lang")
                                .selected_text(&self.settings_language)
                                .show_ui(ui, |ui| {
                                    for lang in &langs {
                                        ui.selectable_value(&mut self.settings_language, lang.to_string(), *lang);
                                    }
                                });
                            ui.end_row();

                            ui.label("Provider:");
                            egui::ComboBox::from_id_salt("provider")
                                .selected_text(&self.settings_provider)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_provider, "cpu".to_owned(), "cpu");
                                    ui.selectable_value(&mut self.settings_provider, "cuda".to_owned(), "cuda");
                                });
                            ui.end_row();

                            ui.label("Num Threads:");
                            ui.add(egui::DragValue::new(&mut self.settings_num_threads).range(1..=16));
                            ui.end_row();

                            ui.label("VAD:");
                            ui.checkbox(&mut self.settings_use_vad, "Enabled");
                            ui.end_row();

                            ui.label("Decoding:");
                            egui::ComboBox::from_id_salt("decoding")
                                .selected_text(&self.settings_decoding_method)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_decoding_method, "greedy_search".to_owned(), "greedy_search");
                                    ui.selectable_value(&mut self.settings_decoding_method, "modified_beam_search".to_owned(), "modified_beam_search");
                                });
                            ui.end_row();

                            ui.label("Inject:");
                            egui::ComboBox::from_id_salt("inject")
                                .selected_text(&self.settings_inject_strategy)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.settings_inject_strategy, "sendinput".to_owned(), "sendinput");
                                    ui.selectable_value(&mut self.settings_inject_strategy, "clipboard".to_owned(), "clipboard");
                                    ui.selectable_value(&mut self.settings_inject_strategy, "auto".to_owned(), "auto");
                                });
                            ui.end_row();

                            ui.label("Key Delay (ms):");
                            ui.add(egui::DragValue::new(&mut self.settings_key_delay_ms).range(0..=100));
                            ui.end_row();

                            ui.label("Restore Clipboard:");
                            ui.checkbox(&mut self.settings_restore_clipboard, "Yes");
                            ui.end_row();

                            ui.label("Text Conversion:");
                            let modes = ["none", "s2t", "t2s"];
                            egui::ComboBox::from_id_salt("conversion")
                                .selected_text(&self.settings_conversion_mode)
                                .show_ui(ui, |ui| {
                                    for mode in &modes {
                                        ui.selectable_value(&mut self.settings_conversion_mode, mode.to_string(), *mode);
                                    }
                                });
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label("Hotkeys (read-only):");
                    ui.label("  Toggle Recording: Ctrl+Shift+F2");
                    ui.label("  Cycle Language: Ctrl+Shift+L");
                    ui.label("  Flush: Ctrl+Shift+Space");

                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            let mut cfg = crate::config::AppConfig::default();
                            cfg.language.language = self.settings_language.clone();
                            cfg.asr.provider = self.settings_provider.clone();
                            cfg.asr.num_threads = self.settings_num_threads;
                            cfg.asr.use_vad = self.settings_use_vad;
                            cfg.asr.decoding_method = self.settings_decoding_method.clone();
                            cfg.injector.strategy = self.settings_inject_strategy.clone();
                            cfg.injector.key_delay_ms = self.settings_key_delay_ms;
                            cfg.injector.restore_clipboard = self.settings_restore_clipboard;
                            cfg.conversion.mode = self.settings_conversion_mode.clone();
                            cfg.ui.language = if self.settings_ui_lang == "Chinese" { "zh".into() } else { "en".into() };
                            cfg.ui.window_x = Some(self.window_x);
                            cfg.ui.window_y = Some(self.window_y);
                            cfg.ui.window_width = Some(self.window_w);
                            cfg.ui.window_height = Some(self.window_h);
                            pending_save = Some(cfg);
                            self.show_settings = false;
                        }
                        if ui.button("Cancel").clicked() {
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

pub fn spawn_gui(
    snapshot: Arc<Mutex<GuiSnapshot>>,
    gui_rx: crossbeam::channel::Receiver<GuiSnapshot>,
    action_tx: crossbeam::channel::Sender<GuiAction>,
    show_overlay: Arc<AtomicBool>,
    initial_pos: Option<egui::Pos2>,
    initial_size: Option<egui::Vec2>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("egui-gui".into())
        .spawn(move || {
            let shared_state = GuiSharedState {
                snapshot,
                gui_rx,
                action_tx,
                show_overlay,
            };
            let mut vp = egui::ViewportBuilder::default()
                .with_min_inner_size([400.0, 300.0])
                .with_title("Nemotron Voice Input");
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
            if let Err(e) = eframe::run_native(
                "Nemotron Voice Input",
                options,
                Box::new(|_cc| Ok(Box::new(GuiApp::new(shared_state, initial_pos, initial_size)))),
            ) {
                tracing::error!("eframe GUI exited with error: {}", e);
            }
        })
        .expect("Failed to spawn GUI thread")
}
