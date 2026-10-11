//! WHAT DRAWS AND WHAT TO DRAW WITH, in Settings -> Viewport.
//!
//! Reported behaviour: on some cards the bodies were not drawn, or the program died at the start, and a person had
//! nothing to change - which card drew, and through which backend, was not said anywhere but in a report. These rows
//! say it, let the person pick the adapter for the starts to come, and, after a start that died drawing sent the
//! program a step down (`safe_graphics`), let them go back to drawing as usual.

/// THE ROWS, under the engine of the viewport.
pub(crate) fn rows(ui: &mut egui::Ui) {
    let drawing = crate::diagnostics::drawing_with().unwrap_or_else(|| crate::i18n::tr("settings-graphics-unknown"));
    ui.label(egui::RichText::new(crate::i18n::tr1("settings-graphics-drawing-with", "what", &drawing)).small());

    let offered = crate::diagnostics::offered_adapters();
    let wanted = crate::safe_graphics::wanted_adapter();
    let auto = crate::i18n::tr("settings-graphics-adapter-auto");
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("settings-graphics-adapter"));
        egui::ComboBox::from_id_salt("settings-graphics-adapter").selected_text(wanted.clone().unwrap_or_else(|| auto.clone())).show_ui(ui, |ui| {
            if ui.selectable_label(wanted.is_none(), &auto).clicked() {
                crate::safe_graphics::want_adapter(None);
            }
            for line in &offered {
                if ui.selectable_label(wanted.as_deref() == Some(line.as_str()), line).clicked() {
                    crate::safe_graphics::want_adapter(Some(line));
                }
            }
        });
    });
    ui.label(egui::RichText::new(crate::i18n::tr("settings-graphics-next-start")).weak().small());

    let step = crate::safe_graphics::step();
    if let Some(key) = crate::safe_graphics::said(step) {
        ui.label(egui::RichText::new(crate::i18n::tr(key)).small());
        if crate::safe_graphics::back_to_normal_asked() {
            ui.label(egui::RichText::new(crate::i18n::tr("settings-graphics-normal-next-start")).weak().small());
        } else if ui.button(crate::i18n::tr("settings-graphics-back-to-normal")).clicked() {
            crate::safe_graphics::back_to_normal();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;

    /// Windows -> Settings, and the rows of the graphics found by the search of the settings, as a person finds a row
    /// below the fold. Answers whether the search field was reached.
    fn to_the_graphics_settings(hand: &mut Hand) -> bool {
        let origin = egui::pos2(0.0, 0.0);
        let opened = hand.press_word(&crate::i18n::tr("menu-windows"), origin) && hand.press_word(&crate::i18n::tr("win-settings"), origin);
        let search = opened && hand.press_word(&crate::i18n::tr("settings-search"), origin);
        if search {
            hand.type_text(crate::i18n::tr("settings-graphics-adapter").trim_end_matches(':'));
        }
        search
    }

    /// THE SETTINGS SAY WHAT DRAWS, PICK THE ADAPTER FOR THE NEXT START, AND TAKE THE PROGRAM BACK TO DRAWING AS
    /// USUAL after a start that died drawing - each by hand, through the window.
    #[test]
    fn the_settings_pick_the_adapter_and_take_the_program_back_to_normal() {
        let _turn = crate::crash::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-graphics-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::crash::use_dir_for_test(Some(&dir));
        let (adapters_before, drawing_before) = (crate::diagnostics::offered_adapters(), crate::diagnostics::drawing_with());
        let vulkan = "Vulkan/DiscreteGpu Radeon (TM) RX 480 Graphics (driver AMD proprietary driver 25.8.1)";
        crate::diagnostics::note_adapters(&[vulkan.to_string(), "Dx12/DiscreteGpu Radeon (TM) RX 480 Series (driver 31.0.21923.11000)".to_string()]);
        crate::diagnostics::note_gpu("wgpu Dx12, Radeon (TM) RX 480 Series (DiscreteGpu)".into());
        crate::safe_graphics::set_step_for_test(crate::safe_graphics::Step::NoAntialiasing);

        let (mut app, _ctx) = running();
        let mut hand = Hand::new(&mut app);
        let origin = egui::pos2(0.0, 0.0);
        let reached = to_the_graphics_settings(&mut hand);
        let says = hand.shows(&crate::i18n::tr1("settings-graphics-drawing-with", "what", "wgpu Dx12, Radeon (TM) RX 480 Series (DiscreteGpu)"));
        let opened = hand.press_word(&crate::i18n::tr("settings-graphics-adapter-auto"), origin);
        let picked = opened && hand.press_word(vulkan, origin);
        let wanted = crate::safe_graphics::wanted_adapter();
        let pressed = hand.press_word(&crate::i18n::tr("settings-graphics-back-to-normal"), origin);
        hand.frame(Vec::new());
        let asked = crate::safe_graphics::back_to_normal_asked();
        let told = hand.shows(&crate::i18n::tr("settings-graphics-normal-next-start"));

        crate::safe_graphics::set_step_for_test(crate::safe_graphics::Step::AsChosen);
        crate::safe_graphics::want_adapter(None);
        crate::diagnostics::put_back_gpu_for_test(adapters_before, drawing_before);
        crate::crash::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(reached, "the search of the settings was not reached");
        assert!(says, "the settings do not say which adapter draws");
        assert!(picked, "the list of adapters was not opened, or the adapter was not in it");
        assert_eq!(wanted.as_deref(), Some(vulkan), "the adapter picked is not kept for the next start");
        assert!(pressed && asked, "the way back to drawing as usual was not offered, or not taken");
        assert!(told, "the settings do not say that drawing as usual comes at the next start");
    }
}
