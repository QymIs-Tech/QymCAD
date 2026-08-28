//! A COMPUTER WITH NO WORKING GRAPHICS DRIVER.
//!
//! Reported behaviour: on Windows 11 with an old card and no driver installed the program starts, blinks
//! and closes. There was nothing in the crash folder, and no falling back to drawing on the processor
//! either. The same version started without trouble on a virtual machine and on a modern card.
//!
//! Three answers, and each is checked here as far as a machine WITH graphics allows. The adapter is chosen
//! by us rather than by the framework, so a processor adapter is taken instead of refused. A start that
//! never reached a window leaves a report, which it did not before - it comes back as an ordinary error
//! and the panic hook never saw it. And the block that report carries names every adapter the machine
//! offered, because "no graphics at all" and "three cards, none of which will draw here" want different
//! answers.
#[cfg(test)]
mod tests {
    use eframe::wgpu::DeviceType;

    /// THE PROCESSOR IS THE LAST CHOICE AND STILL A CHOICE. Ranking it at zero, or leaving it out of the
    /// ordering, is the program that refuses to start rather than starting slowly.
    #[test]
    fn a_processor_is_the_worst_adapter_and_never_a_refusal() {
        use crate::gui::rank_device_type as rank;
        assert!(rank(DeviceType::DiscreteGpu) > rank(DeviceType::IntegratedGpu), "a card of one's own must beat a shared one");
        assert!(rank(DeviceType::IntegratedGpu) > rank(DeviceType::VirtualGpu), "a shared card must beat a virtual one");
        assert!(rank(DeviceType::VirtualGpu) > rank(DeviceType::Other), "a virtual card must beat an unknown one");
        assert!(rank(DeviceType::Other) > rank(DeviceType::Cpu), "the processor is the last resort");
        assert!(rank(DeviceType::Cpu) > 0, "the processor must still be TAKEN when it is the only one on offer");
    }

    /// A START THAT NEVER REACHED A WINDOW LEAVES A REPORT. This is the whole of "there was nothing in the
    /// crash folder": the failure is an ordinary error, not a panic, so nothing was written and nothing
    /// could be sent.
    #[test]
    fn a_start_that_never_happened_is_written_down() {
        // the report directory is one per process - wait for whoever else is redirecting it
        let _turn = crate::crash::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-no-graphics-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::crash::use_dir_for_test(Some(&dir));

        let reason = "no graphics adapter this window can draw on. Offered 0: none at all";
        let path = crate::crash::note_failed_start(reason).expect("a report has to be written");
        let text = std::fs::read_to_string(&path).expect("and to be readable");

        assert!(text.contains(reason), "the report does not say why it would not start: {text}");
        assert!(text.contains("Could not start"), "the report does not say that it never started: {text}");
        assert!(text.starts_with("QymCAD "), "the report does not name the build, which is what a report is for: {text}");
        assert!(
            crate::crash::unseen_reports().iter().any(|p| p == &path),
            "the report is not among the ones the next run shows - written and then invisible is the same as not written"
        );

        crate::crash::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE REPORT NAMES EVERY ADAPTER THAT WAS OFFERED, and says when the processor is the one drawing.
    /// Without these two lines a report from a machine that would not start is a blank page.
    #[test]
    fn the_block_names_what_the_machine_offered() {
        crate::diagnostics::note_adapters(&["Vulkan/Cpu llvmpipe (driver llvmpipe 25.0)".to_string(), "Gl/Other softpipe (driver  )".to_string()]);
        crate::diagnostics::note_drawing_on_the_processor("llvmpipe");

        let b = crate::diagnostics::block();
        assert!(b.contains("Adapters offered: "), "the block does not list what was on offer: {b}");
        assert!(b.contains("llvmpipe"), "the block lost the adapter names: {b}");
        assert!(b.contains("Drawing on the processor: llvmpipe"), "the block does not say the processor is drawing: {b}");
    }

    /// THE PERSON IS TOLD, IN THEIR OWN LANGUAGE, why the model turns like treacle - and told what fixes
    /// it. A slow viewport nobody explained is reported as a fault in the program.
    #[test]
    fn drawing_on_the_processor_is_said_in_words() {
        for lang in ["ru", "en"] {
            crate::i18n::set_language(lang);
            let said = crate::i18n::tr1("gpu-on-the-processor", "name", "llvmpipe");
            assert_ne!(said, "gpu-on-the-processor", "[{lang}] the line is missing from the catalogue");
            assert!(said.contains("llvmpipe"), "[{lang}] the line does not say which one is drawing: {said}");
        }
        crate::i18n::set_language("ru");
    }
}
