//! Localization must not change scenario handles or leave the next app in another locale.

use crate::scenario::{self, Harness};
use serde_json::json;

#[test]
fn japanese_dialog_keeps_english_scenario_handles() {
    let mut h = Harness::new();
    h.app.preferences.language = "ja".into();
    h.app.home.open = false;
    h.call("ui.start", json!({"command":"sketch.create"}));
    h.frames(3);
    assert!(h.widget("平面").is_some(), "the input label must be translated");
    assert!(h.widget("キャンセル").is_some(), "the close button must be translated");
    assert!(!scenario::handles("dialog:Plane").is_empty(), "scenario handles must retain source labels");
    assert_eq!(crate::i18n::tr("ui.preferences"), "Preferences", "the render scope must restore English");

    h.app.preferences.language = "en".into();
    h.frames(3);
    assert!(h.widget("Plane").is_some());
    assert!(h.widget("Cancel").is_some());
}

#[test]
fn language_preference_cancel_restores_previous_locale() {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.call("ui.prefs", json!({"open":"General"}));
    h.call("ui.prefs", json!({"language":"ja"}));
    assert!(h.widget("言語").is_some());
    assert_eq!(h.app.preferences.language, "ja");
    h.call("ui.prefs", json!({"button":"cancel"}));
    assert_eq!(h.app.preferences.language, "en");
    assert_eq!(h.app.tr("ui.preferences"), "Preferences");
}

#[test]
fn translated_dialog_buttons_do_not_wrap() {
    fn cancel_rows(shape: &egui::Shape) -> Option<usize> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == "キャンセル" => Some(text.galley.rows.len()),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(cancel_rows),
            _ => None,
        }
    }
    let mut h = Harness::new();
    crate::fonts::setup_fonts(&h.ctx, "ja");
    h.app.preferences.language = "ja".into();
    h.app.home.open = false;
    h.call("ui.start", json!({"command":"solid.extrude"}));
    let mut out = h.ctx.run_ui(
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))), ..Default::default() },
        |ui| h.app.ui(ui),
    );
    assert_eq!(out.shapes.iter().find_map(|shape| cancel_rows(&shape.shape)), Some(1));
    out.textures_delta.clear();
}
