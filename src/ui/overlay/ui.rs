//! Overlay UI rendering function.
//!
//! Draws a semi-transparent dark rounded-rect frame with centered white text
//! for the floating transcript overlay.

use egui::*;

/// Render a single frame of the overlay.
///
/// * `ui` — egui UI handle (typically from a `CentralPanel`)
/// * `text` — the transcript text to display
/// * `alpha` — background opacity (0.0 .. 1.0)
pub fn render_overlay_frame(ui: &mut Ui, text: &str, alpha: f32) {
    let frame = Frame::NONE
        .fill(Color32::from_black_alpha((alpha * 255.0) as u8))
        .corner_radius(12.0);
    frame.show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(text)
                    .color(Color32::WHITE)
                    .font(FontId::proportional(20.0))
                    .strong(),
            );
            ui.add_space(8.0);
        });
    });
}
