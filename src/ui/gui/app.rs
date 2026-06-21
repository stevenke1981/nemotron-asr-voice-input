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
}

impl GuiApp {
    pub fn new(state: GuiSharedState) -> Self {
        Self {
            state,
            current_snapshot: GuiSnapshot::default(),
            show_settings: false,
            show_overlay_local: false,
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
            egui::Window::new("Settings")
                .open(&mut self.show_settings)
                .show(ctx, |ui| {
                    ui.label("Settings panel placeholder - will be implemented in Phase 2.");
                });
        }

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
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size([800.0, 600.0])
                    .with_min_inner_size([400.0, 300.0])
                    .with_title("Nemotron Voice Input"),
                ..Default::default()
            };
            if let Err(e) = eframe::run_native(
                "Nemotron Voice Input",
                options,
                Box::new(|_cc| Ok(Box::new(GuiApp::new(shared_state)))),
            ) {
                tracing::error!("eframe GUI exited with error: {}", e);
            }
        })
        .expect("Failed to spawn GUI thread")
}
