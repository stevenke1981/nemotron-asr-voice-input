//! Floating transcript overlay module.
//!
//! Provides a transparent always-on-top overlay window that displays the
//! latest ASR transcript text.  Rendered as an egui immediate viewport
//! hosted by the eframe app (no separate winit thread).
//!
//! Auto-fades to 30 % opacity after 5 seconds of no new text.
//! Resets to full opacity on new text or mouse hover.

pub mod ui;

use std::cell::RefCell;
use std::time::Instant;

use egui::{CentralPanel, Context, Vec2, ViewportBuilder, ViewportClass, ViewportId};

thread_local! {
    /// (last text, time of last text change)
    static OVERLAY_STATE: RefCell<(String, Instant)> =
        RefCell::new((String::new(), Instant::now()));
}

/// Show the overlay viewport if `visible` is true.
///
/// Must be called from within the eframe `App::update()` root frame.
pub fn show_overlay_viewport(ctx: &Context, text: &str, visible: bool) {
    if !visible {
        return;
    }

    // Update idle tracker
    let idle_alpha = OVERLAY_STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.0 != text {
            state.0 = text.to_owned();
            state.1 = Instant::now();
            0.85 // full alpha on new text
        } else {
            let elapsed = state.1.elapsed().as_secs_f64();
            if elapsed > 5.0 {
                0.30 // faded
            } else {
                // interpolate smoothly between 0.85 and 0.30
                let t = (elapsed / 5.0) as f32;
                0.85 - (0.85 - 0.30) * t
            }
        }
    });

    let viewport_id = ViewportId::from_hash_of("nemotron-overlay");

    let builder = ViewportBuilder::default()
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_title("Nemotron Overlay");

    ctx.show_viewport_immediate(viewport_id, builder, move |ctx, class| {
        if class == ViewportClass::Immediate {
            let hovered = ctx.is_pointer_over_area();
            // When hovered, always show full alpha
            let alpha = if hovered { 0.85 } else { idle_alpha };
            CentralPanel::default().show(ctx, |ui| {
                ui::render_overlay_frame(ui, text, alpha);
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
