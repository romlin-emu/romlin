//! Fullscreen mode on the library window: the toggle, the filter tabs and playing from the list.

use super::{Controller, RECENT_KEY, SCREEN_LIBRARY, SCREEN_SETTINGS};
use crate::fullscreen::Tab;
use crate::navigation::Dir;
use slint::winit_030::winit::window::WindowId;
use slint::ComponentHandle;

impl Controller {
    /// F11 pressed in `window`. False when the key isn't for the library window: games keep
    /// their own F11, and a game running over it owns the keyboard.
    pub(super) fn fullscreen_key(&self, window: WindowId) -> bool {
        let Some(ui) = self.ui() else { return false };
        let on_library_window = crate::mouse::window_id(ui.window()) == Some(window);
        let on_a_library_screen = (SCREEN_LIBRARY..=SCREEN_SETTINGS).contains(&ui.get_screen());
        if !on_library_window || !on_a_library_screen || self.running.borrow().is_some() {
            return false;
        }
        self.toggle_fullscreen_mode();
        true
    }

    pub(super) fn toggle_fullscreen_mode(&self) {
        let Some(ui) = self.ui() else { return };
        let mut mode = self.fullscreen.get();
        let window_fullscreen = mode.toggle(ui.window().is_fullscreen());
        self.fullscreen.set(mode);
        ui.window().set_fullscreen(window_fullscreen);
        ui.set_fullscreen_layout(self.prefs.get().game_list_layout.id.into());
        ui.set_fullscreen_mode(mode.active());
        self.apply_columns();
        let total = self.library.borrow().games.len();
        let focus = match (mode.active(), self.nav_card.get()) {
            (true, current) => (total > 0).then(|| current.unwrap_or(0).min(total - 1)),
            (false, current) => current,
        };
        self.focus_card(focus);
        self.show_fullscreen_tab();
    }

    fn favorites_key(&self) -> String {
        let favorites = self.shared.store.lock().unwrap().favorites();
        favorites.map_or_else(|| "favorites".into(), |f| f.key)
    }

    fn fullscreen_tab(&self) -> Option<Tab> {
        let filter_on_device = self.library.borrow().filter.downloaded_only;
        Tab::of(
            &self.selected.borrow(),
            &self.favorites_key(),
            filter_on_device,
        )
    }

    fn show_fullscreen_tab(&self) {
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_tab(self.fullscreen_tab().map_or(-1, Tab::index));
        }
    }

    pub(super) fn step_fullscreen_tab(&self, delta: i32) {
        self.choose_fullscreen_tab(Tab::stepped(self.fullscreen_tab(), delta));
    }

    pub(super) fn choose_fullscreen_tab(&self, tab: Tab) {
        let (key, on_device) = match tab {
            Tab::All => ("all".to_string(), false),
            Tab::Favorites => (self.favorites_key(), false),
            Tab::OnDevice => ("all".to_string(), true),
            Tab::Recent => (RECENT_KEY.to_string(), false),
        };
        if self.downloaded_choice.get() != on_device {
            self.set_downloaded_only(on_device);
        }
        self.nav_card.set(None);
        self.select(key);
        let any = !self.library.borrow().games.is_empty();
        self.focus_card(any.then_some(0));
        self.show_fullscreen_tab();
    }

    /// Up or down the list; the layout's rows are one game each.
    pub(super) fn move_fullscreen_row(&self, delta: i32) {
        self.move_card(if delta < 0 { Dir::Up } else { Dir::Down });
    }

    /// Plays the focused game. One that isn't downloaded opens its page, to download from.
    pub(super) fn play_focused(&self) {
        let id = self
            .nav_card
            .get()
            .and_then(|i| self.library.borrow().games.get(i).map(|g| g.id));
        if let Some(id) = id {
            self.open_game(id);
            self.play_game();
        }
    }
}
