//! Floating transcript overlay module.
//!
//! Provides a transparent always-on-top overlay window that displays the
//! latest ASR transcript text.  Rendered as an egui immediate viewport
//! hosted by the eframe app (no separate winit thread).

pub mod ui;

use egui::{CentralPanel, Context, Vec2, ViewportBuilder, ViewportClass, ViewportId};

/// Show the overlay viewport if `visible` is true.
///
/// Must be called from within the eframe `App::update()` root frame.
pub fn show_overlay_viewport(ctx: &Context, text: &str, visible: bool) {
    if !visible {
        return;
    }

    let viewport_id = ViewportId::from_hash_of("nemotron-overlay");

    let builder = ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_title("Nemotron Overlay");

    ctx.show_viewport_immediate(viewport_id, builder, move |ctx, class| {
        if class == ViewportClass::Immediate {
            CentralPanel::default().show(ctx, |ui| {
                let hovered = ctx.is_pointer_over_area();
                ui::render_overlay_frame(ui, text, hovered);
            });
            // Auto-size to content
            let used = ctx.used_size();
            if used.x > 0.0 && used.y > 0.0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                    Vec2::new(used.x + 40.0, used.y + 20.0),
                ));
            }
        }
    });
}
