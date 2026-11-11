//! Fullscreen mode on the library window: the consoles, a console's games, a game's page and the
//! game menu, and what each button or key does on each of them.

use super::game::downloaded_path;
use super::{Controller, RECENT_KEY, SCREEN_GAME, SCREEN_LIBRARY, SCREEN_SETTINGS};
use crate::fullscreen::{self, Action, Mode, Page, Tab, View};
use crate::store::{GameFilter, PlatformItem, Scope};
use crate::{AppWindow, MenuRow, SystemCard};
use slint::winit_030::winit::window::WindowId;
use slint::{ComponentHandle, Image, Model, ModelRc, TimerMode, VecModel};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tr::tr;

const DETAIL_DELAY: Duration = Duration::from_millis(150);
const SETTINGS_PLAYERS: i32 = 2;
const SETTINGS_CONSOLES: i32 = 4;
const RECENT_COVERS: usize = 3;

/// What the game menu can do, in the order its rows are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    Play,
    Resume,
    SaveStates,
    Favorite,
    Core,
    Controls,
    Screenshots,
    Remove,
}

/// What the library looked like before fullscreen mode, put back on leaving it.
struct Restore {
    selected: String,
    on_device: bool,
}

#[derive(Default)]
pub(super) struct State {
    mode: Mode,
    view: View,
    system: usize,
    systems: Vec<PlatformItem>,
    cards: Rc<VecModel<SystemCard>>,
    menu: Vec<MenuItem>,
    menu_focus: usize,
    restore: Option<Restore>,
}

impl Controller {
    pub(super) fn fullscreen_active(&self) -> bool {
        self.fullscreen.borrow().mode.active()
    }

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
        let entering = !self.fullscreen_active();
        let window_fullscreen = self
            .fullscreen
            .borrow_mut()
            .mode
            .toggle(ui.window().is_fullscreen());
        ui.window().set_fullscreen(window_fullscreen);
        if entering {
            self.enter_fullscreen(&ui);
        } else {
            self.leave_fullscreen(&ui);
        }
    }

    fn enter_fullscreen(&self, ui: &AppWindow) {
        let selected = self.selected.borrow().clone();
        let platform = selected.strip_prefix("p:").and_then(|id| id.parse().ok());
        {
            let mut fs = self.fullscreen.borrow_mut();
            fs.restore = Some(Restore {
                selected,
                on_device: self.downloaded_choice.get(),
            });
            fs.menu.clear();
            fs.view = if platform.is_some() {
                View::Games
            } else {
                View::Systems
            };
        }
        ui.set_fullscreen_layout(self.prefs.get().game_list_layout.id.into());
        ui.set_fullscreen_systems(ModelRc::from(self.fullscreen.borrow().cards.clone()));
        ui.set_fullscreen_mode(true);
        self.reload_systems();
        if let Some(id) = platform {
            let index = self
                .fullscreen
                .borrow()
                .systems
                .iter()
                .position(|p| p.id == id);
            if let Some(index) = index {
                self.fullscreen.borrow_mut().system = index;
                ui.set_fullscreen_system(index as i32);
            }
            self.show_system_games(Some(id));
        }
        self.apply_columns();
        self.refresh_fullscreen();
    }

    fn leave_fullscreen(&self, ui: &AppWindow) {
        let restore = self.fullscreen.borrow_mut().restore.take();
        ui.set_fullscreen_mode(false);
        ui.set_fullscreen_menu_open(false);
        ui.set_fullscreen_search_open(false);
        ui.set_fullscreen_launching(false);
        ui.set_fullscreen_full_page(false);
        self.detail_timer.stop();
        self.library.borrow_mut().filter.platform = None;
        self.apply_columns();
        match restore {
            Some(restore) => {
                if self.downloaded_choice.get() != restore.on_device {
                    self.set_downloaded_only(restore.on_device);
                }
                self.select(restore.selected);
            }
            None => self.reload_games(),
        }
        let total = self.library.borrow().games.len();
        let focus = self.nav_card.get().filter(|_| total > 0);
        self.focus_card(focus.map(|i| i.min(total - 1)));
    }

    /// Everything fullscreen mode shows that depends on where the player is.
    pub(super) fn refresh_fullscreen(&self) {
        let Some(ui) = self.ui() else { return };
        if !self.fullscreen_active() {
            return;
        }
        let view = self.fullscreen.borrow().view;
        ui.set_fullscreen_view(match view {
            View::Systems => 0,
            View::Games => 1,
        });
        ui.set_fullscreen_tab(self.fullscreen_tab().map_or(-1, Tab::index));
        self.show_tab_counts();
        self.refresh_chrome();
    }

    /// The heading and the hints, which follow the page and what it is doing.
    pub(super) fn refresh_chrome(&self) {
        let Some(ui) = self.ui() else { return };
        if !self.fullscreen_active() {
            return;
        }
        ui.set_fullscreen_heading(self.fullscreen_heading(&ui).into());
        self.show_fullscreen_hints();
    }

    fn fullscreen_heading(&self, ui: &AppWindow) -> String {
        if self.preparing.get() {
            return tr!("Launching");
        }
        if ui.get_screen() == SCREEN_GAME {
            return ui.get_game_platform().into();
        }
        let platform = self.library.borrow().filter.platform;
        match (self.fullscreen.borrow().view, platform) {
            (View::Systems, _) => tr!("Systems"),
            (View::Games, Some(id)) => self
                .fullscreen
                .borrow()
                .systems
                .iter()
                .find(|p| p.id == id)
                .map_or_else(|| tr!("All games"), |p| p.name.clone()),
            (View::Games, None) if !self.library.borrow().filter.search.is_empty() => tr!("Search"),
            (View::Games, None) => tr!("All games"),
        }
    }

    /// The library's data changed under fullscreen mode: a sync, a favourite, a download.
    pub(super) fn fullscreen_library_changed(&self) {
        if !self.fullscreen_active() {
            return;
        }
        self.reload_systems();
        self.refresh_fullscreen();
    }

    pub(super) fn fullscreen_games_reloaded(&self) {
        if self.fullscreen_active() && self.fullscreen.borrow().view == View::Games {
            self.schedule_detail();
        }
    }

    /// Whether a game is starting; fullscreen mode shows its launch screen meanwhile.
    pub(super) fn set_preparing(&self, preparing: bool) {
        self.preparing.set(preparing);
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_launching(preparing && self.fullscreen_active());
        }
        self.refresh_chrome();
    }

    // ---- Input -----------------------------------------------------------------------------

    /// A key reached fullscreen mode's focus scope; whether it was one of ours.
    pub(super) fn fullscreen_key_pressed(&self, text: &str, ctrl: bool) -> bool {
        let Some(action) = Action::from_key(text, ctrl) else {
            return false;
        };
        let action = match action {
            Action::Left if crate::i18n::rtl() => Action::Right,
            Action::Right if crate::i18n::rtl() => Action::Left,
            other => other,
        };
        self.fullscreen_action(action);
        true
    }

    /// A controller button while fullscreen mode is showing; whether it was one of ours.
    pub(super) fn fullscreen_button(&self, button: u32) -> bool {
        let Some(ui) = self.ui() else { return false };
        let Some(action) = Action::from_button(self.fullscreen_page(&ui), button) else {
            return false;
        };
        self.fullscreen_action(action);
        true
    }

    fn fullscreen_page(&self, ui: &AppWindow) -> Page {
        if ui.get_screen() == SCREEN_GAME {
            return Page::Game;
        }
        match self.fullscreen.borrow().view {
            View::Systems => Page::Systems,
            View::Games => Page::Games,
        }
    }

    fn fullscreen_action(&self, action: Action) {
        let Some(ui) = self.ui() else { return };
        if self.preparing.get() {
            return;
        }
        if ui.get_fullscreen_menu_open() {
            return self.menu_action(action);
        }
        if ui.get_fullscreen_search_open() {
            match action {
                Action::Back => self.close_search(true),
                Action::Accept => self.close_search(false),
                _ => {}
            }
            return;
        }
        match self.fullscreen_page(&ui) {
            Page::Systems => self.systems_action(action),
            Page::Games => self.games_action(action),
            Page::Game => self.game_page_action(&ui, action),
        }
    }

    fn systems_action(&self, action: Action) {
        match action {
            Action::Left | Action::Prev => self.step_system(-1),
            Action::Right | Action::Next => self.step_system(1),
            Action::Accept => {
                let index = self.fullscreen.borrow().system;
                self.open_system(index);
            }
            Action::Back => self.toggle_fullscreen_mode(),
            Action::Menu => self.open_settings(),
            Action::Search => self.open_search(),
            Action::Random => self.random_game(),
            _ => {}
        }
    }

    fn games_action(&self, action: Action) {
        match action {
            Action::Up => self.move_card(crate::navigation::Dir::Up),
            Action::Down => self.move_card(crate::navigation::Dir::Down),
            Action::Left | Action::Prev => self.step_fullscreen_tab(-1),
            Action::Right | Action::Next => self.step_fullscreen_tab(1),
            Action::JumpBack => self.jump_letter(false),
            Action::JumpForward => self.jump_letter(true),
            Action::Accept => self.play_focused(),
            Action::Back => self.back_to_systems(),
            Action::Menu => self.open_menu(),
            Action::Favorite => self.favorite_focused(),
            Action::Resume => self.resume_focused(),
            Action::Search => self.open_search(),
            Action::Random => self.random_game(),
            Action::Download => {}
        }
        if matches!(action, Action::Up | Action::Down) {
            self.schedule_detail();
        }
    }

    fn game_page_action(&self, ui: &AppWindow, action: Action) {
        match action {
            Action::Accept if ui.get_game_downloaded() => self.play_game(),
            Action::Accept if !ui.get_game_busy() => self.download_game(),
            Action::Download if ui.get_game_busy() => self.cancel_download(),
            Action::Download if !ui.get_game_downloaded() => self.download_game(),
            Action::Resume => self.resume_current(),
            Action::Favorite => {
                if ui.get_can_edit_collections() {
                    self.toggle_favorite();
                }
            }
            Action::Menu => self.open_menu(),
            Action::Back => self.back_to_library(),
            Action::Left | Action::Prev => self.step_game(-1),
            Action::Right | Action::Next => self.step_game(1),
            _ => {}
        }
    }

    // ---- Consoles --------------------------------------------------------------------------

    fn reload_systems(&self) {
        let Some(ui) = self.ui() else { return };
        let (platforms, on_device) = {
            let store = self.shared.store.lock().unwrap();
            (store.platforms(self.offline.get()), store.platforms(true))
        };
        let mut missing = Vec::new();
        let cards: Vec<SystemCard> = platforms
            .iter()
            .map(|p| {
                let icon = self
                    .shared
                    .covers
                    .cached_icon(&p.slug)
                    .and_then(|path| Image::load_from_path(&path).ok());
                if icon.is_none() && self.icon_requests.borrow_mut().insert(p.slug.clone()) {
                    missing.push(p.slug.clone());
                }
                let (r, g, b) = fullscreen::system_tint(&p.slug);
                SystemCard {
                    key: format!("p:{}", p.id).into(),
                    name: p.name.clone().into(),
                    has_icon: icon.is_some(),
                    icon: icon.unwrap_or_default(),
                    games: p.count as i32,
                    on_device: on_device
                        .iter()
                        .find(|d| d.id == p.id)
                        .map_or(0, |d| d.count as i32),
                    tint: slint::Color::from_rgb_u8(r, g, b),
                    recent: ModelRc::default(),
                    more: 0,
                    last_played: "".into(),
                }
            })
            .collect();
        let system = {
            let mut fs = self.fullscreen.borrow_mut();
            fs.system = fs.system.min(cards.len().saturating_sub(1));
            fs.systems = platforms;
            fs.cards.set_vec(cards);
            fs.system
        };
        ui.set_fullscreen_system(system as i32);
        self.fetch_icons(missing);
        self.load_system_extras();
    }

    /// The focused console's recent games: the covers already on this computer, and when the
    /// last one was played.
    fn load_system_extras(&self) {
        let (platform, index, cards) = {
            let fs = self.fullscreen.borrow();
            let Some(platform) = fs.systems.get(fs.system).cloned() else {
                return;
            };
            (platform, fs.system, fs.cards.clone())
        };
        let Some(mut card) = cards.row_data(index) else {
            return;
        };
        let (recent, last) = {
            let store = self.shared.store.lock().unwrap();
            let recent = store.games(&GameFilter {
                scope: Scope::Recent,
                platform: Some(platform.id),
                downloaded_only: self.offline.get(),
                ..GameFilter::default()
            });
            let last = recent
                .first()
                .and_then(|g| Some((g.title.clone(), store.last_played(g.id)?)));
            (recent, last)
        };
        let covers: Vec<Image> = recent
            .iter()
            .filter_map(|g| {
                let path = self.shared.covers.path_for(g.id, g.cover.as_deref()?);
                Image::load_from_path(&path).ok()
            })
            .take(RECENT_COVERS)
            .collect();
        card.more = (platform.count - covers.len() as i64).max(0) as i32;
        card.recent = ModelRc::new(VecModel::from(covers));
        card.last_played = last
            .map(|(title, ms)| {
                let when = UNIX_EPOCH + Duration::from_millis(u64::try_from(ms).unwrap_or(0));
                format!("{title} · {}", crate::slots::when(when, SystemTime::now()))
            })
            .unwrap_or_default()
            .into();
        cards.set_row_data(index, card);
    }

    fn step_system(&self, delta: i32) {
        let len = self.fullscreen.borrow().systems.len() as i32;
        if len == 0 {
            return;
        }
        let system = {
            let mut fs = self.fullscreen.borrow_mut();
            fs.system = (fs.system as i32 + delta).rem_euclid(len) as usize;
            fs.system
        };
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_system(system as i32);
        }
        self.load_system_extras();
    }

    pub(super) fn choose_system(&self, index: usize) {
        let focused = self.fullscreen.borrow().system;
        if index == focused {
            return self.open_system(index);
        }
        let len = self.fullscreen.borrow().systems.len();
        if index < len {
            self.fullscreen.borrow_mut().system = index;
            if let Some(ui) = self.ui() {
                ui.set_fullscreen_system(index as i32);
            }
            self.load_system_extras();
        }
    }

    fn open_system(&self, index: usize) {
        let Some(platform) = self.fullscreen.borrow().systems.get(index).map(|p| p.id) else {
            return;
        };
        self.fullscreen.borrow_mut().system = index;
        self.fullscreen.borrow_mut().view = View::Games;
        self.show_system_games(Some(platform));
        self.refresh_fullscreen();
    }

    /// Shows `platform`'s games (every console's, when there is none) from the All tab.
    fn show_system_games(&self, platform: Option<i64>) {
        self.library.borrow_mut().filter.platform = platform;
        self.fullscreen.borrow_mut().view = View::Games;
        self.choose_fullscreen_tab(Tab::All);
    }

    fn back_to_systems(&self) {
        self.close_search(true);
        self.library.borrow_mut().filter.platform = None;
        self.fullscreen.borrow_mut().view = View::Systems;
        self.detail_timer.stop();
        if self.downloaded_choice.get() {
            self.set_downloaded_only(false);
        }
        self.select("all".into());
        self.reload_systems();
        self.refresh_fullscreen();
    }

    // ---- Games -----------------------------------------------------------------------------

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

    fn show_tab_counts(&self) {
        let Some(ui) = self.ui() else { return };
        if self.fullscreen.borrow().view != View::Games {
            return;
        }
        let platform = self.library.borrow().filter.platform;
        let favorites = self.favorites_key();
        let store = self.shared.store.lock().unwrap();
        let count = |scope: Scope, on_device: bool| {
            store
                .games(&GameFilter {
                    scope,
                    platform,
                    downloaded_only: on_device || self.offline.get(),
                    ..GameFilter::default()
                })
                .len() as i32
        };
        let counts = [
            count(Scope::All, false),
            count(Scope::Collection(favorites), false),
            count(Scope::All, true),
            count(Scope::Recent, false),
        ];
        ui.set_fullscreen_counts(ModelRc::new(VecModel::from(counts.to_vec())));
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
        self.schedule_detail();
        self.refresh_fullscreen();
    }

    /// A game clicked in the list.
    pub(super) fn choose_row(&self, index: usize) {
        if index < self.library.borrow().games.len() {
            self.focus_card(Some(index));
            self.schedule_detail();
        }
    }

    fn jump_letter(&self, forward: bool) {
        let jump = {
            let lib = self.library.borrow();
            let titles: Vec<&str> = lib.games.iter().map(|g| g.title.as_str()).collect();
            let from = self.nav_card.get().unwrap_or(0);
            fullscreen::letter_jump(&titles, from, forward)
        };
        if let Some(index) = jump {
            self.focus_card(Some(index));
            self.schedule_detail();
        }
    }

    fn focused_game_id(&self) -> Option<i64> {
        self.nav_card
            .get()
            .and_then(|i| self.library.borrow().games.get(i).map(|g| g.id))
    }

    /// Fills the details beside the list once the focus has settled, so scrolling past games
    /// doesn't load each of them.
    fn schedule_detail(&self) {
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_detail_ready(false);
        }
        self.detail_timer
            .start(TimerMode::SingleShot, DETAIL_DELAY, || {
                super::with_controller(|c| c.load_focused_detail());
            });
    }

    fn load_focused_detail(&self) {
        let Some(id) = self.focused_game_id() else {
            return;
        };
        self.ensure_loaded(id);
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_detail_ready(true);
        }
    }

    fn ensure_loaded(&self, id: i64) -> bool {
        self.current_game_id() == Some(id) || self.load_game(id)
    }

    /// Plays the focused game. One that isn't downloaded opens its page, to download from.
    pub(super) fn play_focused(&self) {
        let Some(id) = self.focused_game_id() else {
            return;
        };
        if !self.ensure_loaded(id) {
            return;
        }
        if self.ui().is_some_and(|ui| ui.get_game_downloaded()) {
            self.play_game();
        } else {
            self.open_game_page();
        }
    }

    pub(super) fn open_game_page(&self) {
        if let Some(ui) = self.ui() {
            ui.set_screen(SCREEN_GAME);
        }
        self.refresh_fullscreen();
    }

    pub(super) fn open_focused_page(&self) {
        let Some(id) = self.focused_game_id() else {
            return;
        };
        if self.ensure_loaded(id) {
            self.open_game_page();
        }
    }

    pub(super) fn resume_focused(&self) {
        if self.ui().is_some_and(|ui| ui.get_screen() == SCREEN_GAME) {
            return self.resume_current();
        }
        let Some(id) = self.focused_game_id() else {
            return;
        };
        if self.ensure_loaded(id) {
            self.resume_current();
        }
    }

    fn resume_current(&self) {
        let Some(detail) = self.current_game() else {
            return;
        };
        if let Some(info) = self.latest_state(&detail) {
            self.play_game_from(Some(info.slot));
        }
    }

    fn favorite_focused(&self) {
        let Some(id) = self.focused_game_id() else {
            return;
        };
        let can_edit = self.ui().is_some_and(|ui| ui.get_can_edit_collections());
        if can_edit && self.ensure_loaded(id) {
            self.toggle_favorite();
        }
    }

    /// The previous or next game of the list, from a game's page.
    fn step_game(&self, delta: i32) {
        let (total, current) = (self.library.borrow().games.len(), self.nav_card.get());
        let Some(current) = current.filter(|_| total > 1) else {
            return;
        };
        let next = (current as i32 + delta).rem_euclid(total as i32) as usize;
        self.focus_card(Some(next));
        if let Some(id) = self.focused_game_id() {
            self.load_game(id);
        }
        self.refresh_fullscreen();
    }

    fn random_game(&self) {
        let games = {
            let filter = GameFilter {
                downloaded_only: self.offline.get(),
                platform: self.library.borrow().filter.platform,
                ..GameFilter::default()
            };
            if self.fullscreen.borrow().view == View::Games {
                self.library.borrow().games.iter().map(|g| g.id).collect()
            } else {
                self.shared
                    .store
                    .lock()
                    .unwrap()
                    .games(&filter)
                    .iter()
                    .map(|g| g.id)
                    .collect::<Vec<_>>()
            }
        };
        let Some(id) =
            super::surprise::pick(&games, self.surprise_last.get(), super::surprise::roll())
        else {
            return;
        };
        self.surprise_last.set(Some(id));
        if self.fullscreen.borrow().view == View::Systems {
            let platform = self
                .shared
                .store
                .lock()
                .unwrap()
                .game(id)
                .map(|g| g.platform_id);
            let index = self
                .fullscreen
                .borrow()
                .systems
                .iter()
                .position(|p| Some(p.id) == platform);
            let Some(index) = index else { return };
            self.open_system(index);
        }
        let position = self.library.borrow().games.iter().position(|g| g.id == id);
        if let Some(position) = position {
            self.focus_card(Some(position));
            self.schedule_detail();
        }
    }

    // ---- Search ----------------------------------------------------------------------------

    fn open_search(&self) {
        if self.fullscreen.borrow().view == View::Systems {
            self.show_system_games(None);
            self.refresh_fullscreen();
        }
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_search_open(true);
        }
    }

    pub(super) fn submit_search(&self) {
        self.close_search(false);
    }

    /// Closes the search field, dropping what was typed when `clear`.
    fn close_search(&self, clear: bool) {
        let Some(ui) = self.ui() else { return };
        ui.set_fullscreen_search_open(false);
        if clear && !self.library.borrow().filter.search.is_empty() {
            ui.set_search("".into());
            self.set_search(String::new());
        }
    }

    // ---- Game menu -------------------------------------------------------------------------

    fn open_menu(&self) {
        let Some(ui) = self.ui() else { return };
        if ui.get_screen() != SCREEN_GAME {
            let Some(id) = self.focused_game_id() else {
                return;
            };
            if !self.ensure_loaded(id) {
                return;
            }
        }
        self.fullscreen.borrow_mut().menu_focus = 0;
        self.show_menu(&ui);
        ui.set_fullscreen_menu_open(true);
        self.show_fullscreen_hints();
    }

    fn close_menu(&self) {
        if let Some(ui) = self.ui() {
            ui.set_fullscreen_menu_open(false);
        }
        self.show_fullscreen_hints();
    }

    pub(super) fn dismiss_menu(&self) {
        self.close_menu();
    }

    fn show_menu(&self, ui: &AppWindow) {
        let Some(detail) = self.current_game() else {
            return;
        };
        let downloaded = downloaded_path(&detail).is_some();
        let resume = self.latest_state(&detail);
        let states = ui.get_game_save_states();
        let shots = ui.get_game_shots().row_count();
        let now = SystemTime::now();
        let mut items = Vec::new();
        let mut row = |item: MenuItem, glyph: &str, label: String, detail: String, danger: bool| {
            items.push((
                item,
                MenuRow {
                    glyph: glyph.into(),
                    label: label.into(),
                    detail: detail.into(),
                    danger,
                },
            ));
        };
        if downloaded {
            row(MenuItem::Play, "▶", tr!("Play"), String::new(), false);
        } else if ui.get_game_can_download() {
            row(
                MenuItem::Play,
                "↓",
                tr!("Download"),
                ui.get_game_size().into(),
                false,
            );
        }
        if let (true, Some(info)) = (downloaded, resume) {
            let when = info.saved_at.map(|at| crate::slots::when(at, now));
            row(
                MenuItem::Resume,
                "↺",
                tr!("Resume from save state"),
                [Some(info.label()), when]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
                false,
            );
        }
        if downloaded || states > 0 {
            row(
                MenuItem::SaveStates,
                "☰",
                tr!("Manage save states"),
                if states > 0 {
                    states.to_string()
                } else {
                    String::new()
                },
                false,
            );
        }
        if ui.get_can_edit_collections() {
            let label = if ui.get_game_favorite() {
                tr!("Remove from favorites")
            } else {
                tr!("Add to favorites")
            };
            row(MenuItem::Favorite, "♥", label, String::new(), false);
        }
        row(
            MenuItem::Core,
            "⚙",
            tr!("Emulator core"),
            ui.get_game_core().into(),
            false,
        );
        row(
            MenuItem::Controls,
            "◧",
            tr!("Controls & hotkeys"),
            String::new(),
            false,
        );
        if shots > 0 {
            row(
                MenuItem::Screenshots,
                "▣",
                tr!("Screenshots"),
                shots.to_string(),
                false,
            );
        }
        if downloaded {
            row(
                MenuItem::Remove,
                "✕",
                tr!("Remove from device"),
                ui.get_game_size().into(),
                true,
            );
        }
        let (menu, rows): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        self.fullscreen.borrow_mut().menu = menu;
        ui.set_fullscreen_menu(ModelRc::new(VecModel::from(rows)));
        ui.set_fullscreen_menu_focus(self.fullscreen.borrow().menu_focus as i32);
    }

    fn menu_action(&self, action: Action) {
        let Some(ui) = self.ui() else { return };
        let len = self.fullscreen.borrow().menu.len();
        match action {
            Action::Up | Action::Down if len > 0 => {
                let delta = if action == Action::Up { -1 } else { 1 };
                let focus = {
                    let mut fs = self.fullscreen.borrow_mut();
                    fs.menu_focus = (fs.menu_focus as i32 + delta).rem_euclid(len as i32) as usize;
                    fs.menu_focus
                };
                ui.set_fullscreen_menu_focus(focus as i32);
            }
            Action::Accept => {
                let focus = self.fullscreen.borrow().menu_focus;
                self.run_menu(focus);
            }
            Action::Back | Action::Menu => self.close_menu(),
            _ => {}
        }
    }

    pub(super) fn run_menu(&self, index: usize) {
        let Some(ui) = self.ui() else { return };
        let Some(item) = self.fullscreen.borrow().menu.get(index).copied() else {
            return;
        };
        if item == MenuItem::Favorite {
            self.toggle_favorite();
            self.show_menu(&ui);
            return;
        }
        self.close_menu();
        match item {
            MenuItem::Play if ui.get_game_downloaded() => self.play_game(),
            MenuItem::Play => self.download_game(),
            MenuItem::Resume => self.resume_current(),
            MenuItem::SaveStates | MenuItem::Screenshots => {
                ui.set_fullscreen_full_page(true);
                ui.set_screen(SCREEN_GAME);
            }
            MenuItem::Core => self.open_settings_section(SETTINGS_CONSOLES),
            MenuItem::Controls => self.open_settings_section(SETTINGS_PLAYERS),
            MenuItem::Remove => self.confirm_remove_game(),
            MenuItem::Favorite => {}
        }
        self.refresh_fullscreen();
    }

    fn open_settings_section(&self, section: i32) {
        let Some(ui) = self.ui() else { return };
        self.open_settings();
        ui.set_settings_section(section);
        self.settings_section_changed(section);
    }
}
