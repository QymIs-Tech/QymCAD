//! THE MENU BAR IS WALKED BY HOVER once it has been opened.
//!
//! Reported behaviour: "when I click a menu and move the mouse to the next menu, it does not expand". The
//! first menu is opened by a click; after that, the pointer passing over another title of the same bar opens
//! that one in its place, until a click outside or a chosen item closes the menu - as menu bars behave
//! everywhere else.
//!
//! egui's `MenuBar` does not do this itself: every `MenuButton` toggles its own popup on a click and knows
//! nothing of its neighbours.

/// A drop-down menu of the menu bar that hands the open menu over to itself when the pointer comes to it.
pub(crate) trait BarMenu {
    fn bar_menu_button<'a, R>(
        &mut self,
        atoms: impl egui::IntoAtoms<'a>,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>>;
}

impl BarMenu for egui::Ui {
    fn bar_menu_button<'a, R>(
        &mut self,
        atoms: impl egui::IntoAtoms<'a>,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let inner = self.menu_button(atoms, add_contents);
        let ctx = self.ctx().clone();
        let popup = egui::Popup::default_response_id(&inner.response);
        // THE MENUS OF THIS BAR, remembered across frames: a title drawn before the open menu must still
        // know that menu is open, and an unrelated popup elsewhere (a combo box) must not count as one.
        let key = self.id().with("bar_menus");
        let siblings = ctx.data_mut(|d| {
            let ids = d.get_temp_mut_or_default::<Vec<egui::Id>>(key);
            if !ids.contains(&popup) {
                ids.push(popup);
            }
            ids.clone()
        });
        let another_open = siblings.iter().any(|&id| id != popup && egui::Popup::is_id_open(&ctx, id));
        // A click on this title is already toggling it through `menu_button`; opening it here as well would
        // let the toggle close it again on the same frame.
        if another_open && inner.response.hovered() && !inner.response.clicked() {
            // `open_id` closes every other popup, so the menu is handed over rather than doubled
            egui::Popup::open_id(&ctx, popup);
            ctx.request_repaint();
        }
        inner
    }
}

#[cfg(test)]
mod tests {
    use super::BarMenu;

    const FILE: &str = "File";
    const EDIT: &str = "Edit";

    /// A bar of two menus driven by real pointer input, one frame per step.
    struct Bar {
        ctx: egui::Context,
        file: egui::Rect,
        edit: egui::Rect,
        file_popup: egui::Id,
        edit_popup: egui::Id,
    }

    impl Bar {
        fn new() -> Self {
            let mut bar = Bar {
                ctx: egui::Context::default(),
                file: egui::Rect::NOTHING,
                edit: egui::Rect::NOTHING,
                file_popup: egui::Id::NULL,
                edit_popup: egui::Id::NULL,
            };
            bar.frame(Vec::new());
            bar
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
            let (mut file, mut edit) = (None, None);
            let _ = self.ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    file = Some(ui.bar_menu_button(FILE, |ui| ui.label("new")).response);
                    edit = Some(ui.bar_menu_button(EDIT, |ui| ui.label("undo")).response);
                });
            });
            let (file, edit) = (file.expect("the File title is drawn"), edit.expect("the Edit title is drawn"));
            self.file = file.rect;
            self.edit = edit.rect;
            self.file_popup = egui::Popup::default_response_id(&file);
            self.edit_popup = egui::Popup::default_response_id(&edit);
        }

        /// The pointer comes to a point and stays there for a few frames.
        fn hover(&mut self, at: egui::Pos2) {
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(Vec::new());
            self.frame(Vec::new());
        }

        fn click(&mut self, at: egui::Pos2) {
            self.hover(at);
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            self.frame(vec![button(true)]);
            self.frame(vec![button(false)]);
            self.frame(Vec::new());
        }

        fn open(&self, popup: egui::Id) -> bool {
            egui::Popup::is_id_open(&self.ctx, popup)
        }
    }

    /// ONCE A MENU IS OPEN, THE NEXT TITLE OPENS BY HOVER ALONE, and the first one gives way to it.
    #[test]
    fn an_open_menu_moves_to_the_title_under_the_pointer() {
        let mut bar = Bar::new();
        bar.click(bar.file.center());
        assert!(bar.open(bar.file_popup), "a click on File must open it");
        bar.hover(bar.edit.center());
        assert!(bar.open(bar.edit_popup), "with File open, bringing the pointer to Edit must open Edit");
        assert!(!bar.open(bar.file_popup), "only one menu of the bar is open at a time: File must give way to Edit");
        bar.hover(bar.file.center());
        assert!(bar.open(bar.file_popup), "and back again: the pointer returning to File opens File");
    }

    /// THE FIRST MENU STILL WANTS A CLICK: hover alone over a closed bar opens nothing.
    #[test]
    fn a_closed_bar_does_not_open_by_hover() {
        let mut bar = Bar::new();
        bar.hover(bar.file.center());
        bar.hover(bar.edit.center());
        assert!(!bar.open(bar.file_popup) && !bar.open(bar.edit_popup), "hovering a closed bar must not open a menu");
    }

    /// THE REAL MENU BAR IS BUILT FROM THESE MENUS rather than from egui's plain ones: the checks above
    /// prove nothing about the application if its bar does not go through `bar_menu_button`.
    #[test]
    fn the_application_bar_uses_the_walking_menus() {
        let panels = crate::gui::panels_source::PANELS;
        for key in ["menu-file", "menu-edit", "menu-view", "menu-windows", "menu-help"] {
            assert!(
                panels.contains(&format!(r#"bar_menu_button(qymcad_i18n::tr("{key}")"#)),
                "the \"{key}\" menu of the bar must be a `bar_menu_button`, or hover does not move the open menu to it"
            );
        }
    }

    /// A CLICK OUTSIDE ENDS THE WALK: after it, hover opens nothing again.
    #[test]
    fn a_click_outside_ends_the_walk() {
        let mut bar = Bar::new();
        bar.click(bar.file.center());
        bar.click(egui::pos2(700.0, 500.0));
        assert!(!bar.open(bar.file_popup), "a click outside must close the menu");
        bar.hover(bar.edit.center());
        assert!(!bar.open(bar.edit_popup), "after a click outside, hovering Edit must not open it");
    }
}
