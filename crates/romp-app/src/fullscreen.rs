//! Fullscreen mode: a controller-friendly presentation of the library.
//!
//! Three small pieces live here, none of which touch the UI: the game list layouts the mode can
//! show, the filter tabs above them, and the state of the mode itself.
//!
//! To add a layout, add a [`GameListLayout`] to [`LAYOUTS`] and a Slint component in
//! `ui/fullscreen.slint`, then show it for its id in `FullscreenView` (`ui/app.slint`).

use crate::app::RECENT_KEY;
use crate::gettext_noop;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A way of laying out the game list in fullscreen mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameListLayout {
    /// Saved in the preferences and matched by the Slint `FullscreenView`; never change one.
    pub id: &'static str,
    /// What a picker shows, as a gettext key.
    pub name: &'static str,
    /// Games per row of the shared `rows` model: the layout's own grid width.
    pub columns: usize,
}

pub const LIST_DETAIL: GameListLayout = GameListLayout {
    id: "list-detail",
    name: gettext_noop!("List + Detail"),
    columns: 1,
};

pub const LAYOUTS: [GameListLayout; 1] = [LIST_DETAIL];

impl GameListLayout {
    /// The layout with this id, or the default when it is unknown (a newer version saved it).
    pub fn by_id(id: &str) -> Self {
        LAYOUTS
            .into_iter()
            .find(|layout| layout.id == id)
            .unwrap_or_default()
    }
}

impl Default for GameListLayout {
    fn default() -> Self {
        LIST_DETAIL
    }
}

impl Serialize for GameListLayout {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id)
    }
}

impl<'de> Deserialize<'de> for GameListLayout {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        Ok(Self::by_id(&id))
    }
}

/// The filters above the list, each an existing library filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    All,
    Favorites,
    OnDevice,
    Recent,
}

impl Tab {
    const ALL: [Self; 4] = [Self::All, Self::Favorites, Self::OnDevice, Self::Recent];

    pub fn from_index(index: i32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .unwrap_or_default()
    }

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as i32
    }

    /// The tab showing `selected` (a sidebar key) with or without downloaded-only on, if any: a
    /// console or collection picked in the sidebar belongs to none of them.
    pub fn of(selected: &str, favorites_key: &str, on_device: bool) -> Option<Self> {
        match (selected, on_device) {
            ("all", false) => Some(Self::All),
            ("all", true) => Some(Self::OnDevice),
            (RECENT_KEY, _) => Some(Self::Recent),
            (key, _) if key == favorites_key => Some(Self::Favorites),
            _ => None,
        }
    }

    /// The tab `delta` places from `current`, wrapping at either end; with none showing, the
    /// first going forward and the last going back.
    pub fn stepped(current: Option<Self>, delta: i32) -> Self {
        match current {
            Some(tab) => Self::from_index((tab.index() + delta).rem_euclid(Self::ALL.len() as i32)),
            None if delta < 0 => Self::Recent,
            None => Self::All,
        }
    }
}

/// Whether fullscreen mode is on, and what the window looked like before it was.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mode {
    active: bool,
    window_was_fullscreen: bool,
}

impl Mode {
    pub fn active(self) -> bool {
        self.active
    }

    /// Flips the mode and returns whether the window should be fullscreen afterwards. A window
    /// that was already fullscreen (game mode, or the system's own button) stays so on leaving.
    pub fn toggle(&mut self, window_is_fullscreen: bool) -> bool {
        if self.active {
            self.active = false;
            self.window_was_fullscreen
        } else {
            self.active = true;
            self.window_was_fullscreen = window_is_fullscreen;
            true
        }
    }
}

/// What fullscreen mode is showing. The game page is the app's own game screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Systems,
    Games,
}

/// Where the player is: one of the views, or a game's page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Systems,
    Games,
    Game,
}

/// Something the player asked for, from a controller button or a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Accept,
    Back,
    /// The previous system, list or game.
    Prev,
    Next,
    JumpBack,
    JumpForward,
    Menu,
    Favorite,
    Resume,
    Random,
    Search,
    Download,
}

impl Action {
    /// What a menu button does on `page`; the buttons are the ones the rest of the app names.
    pub fn from_button(page: Page, button: u32) -> Option<Self> {
        use crate::input::{A, B, DOWN, L, L2, LEFT, R, R2, RIGHT, START, UP, X, Y};
        Some(match (button, page) {
            (UP, _) => Self::Up,
            (DOWN, _) => Self::Down,
            (LEFT, _) => Self::Left,
            (RIGHT, _) => Self::Right,
            (A, _) => Self::Accept,
            (B, _) => Self::Back,
            (L, _) => Self::Prev,
            (R, _) => Self::Next,
            (START, _) => Self::Menu,
            (L2, Page::Games) => Self::JumpBack,
            (R2, Page::Games) => Self::JumpForward,
            (X, Page::Systems) => Self::Search,
            (Y, Page::Systems) => Self::Random,
            (X, Page::Games | Page::Game) => Self::Favorite,
            (Y, Page::Games) => Self::Resume,
            (Y, Page::Game) => Self::Download,
            _ => return None,
        })
    }

    /// What a key does. `text` is the key's text as Slint reports it.
    pub fn from_key(text: &str, ctrl: bool) -> Option<Self> {
        use slint::platform::Key;
        let mut chars = text.chars();
        let (first, rest) = (chars.next()?, chars.next());
        let named = [
            (Key::UpArrow, Self::Up),
            (Key::DownArrow, Self::Down),
            (Key::LeftArrow, Self::Left),
            (Key::RightArrow, Self::Right),
            (Key::Return, Self::Accept),
            (Key::Escape, Self::Back),
            (Key::Backspace, Self::Back),
            (Key::PageUp, Self::Prev),
            (Key::PageDown, Self::Next),
            (Key::Home, Self::JumpBack),
            (Key::End, Self::JumpForward),
        ];
        if let Some((_, action)) = named.iter().find(|(key, _)| char::from(*key) == first) {
            return Some(*action);
        }
        if rest.is_some() {
            return None;
        }
        Some(match first {
            '/' => Self::Search,
            'f' | 'F' if ctrl => Self::Search,
            'm' | 'M' => Self::Menu,
            'f' | 'F' => Self::Favorite,
            'r' | 'R' => Self::Resume,
            'x' | 'X' => Self::Random,
            'd' | 'D' => Self::Download,
            _ => return None,
        })
    }
}

/// The first game of the next letter, or of this one when going back from partway through it.
/// Games starting with a digit or a symbol share one group, as they do in a sorted list.
pub fn letter_jump<S: AsRef<str>>(titles: &[S], from: usize, forward: bool) -> Option<usize> {
    let group = |i: usize| {
        titles[i]
            .as_ref()
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .filter(|c| c.is_alphabetic())
    };
    let from = from.min(titles.len().checked_sub(1)?);
    let current = group(from);
    if forward {
        return (from + 1..titles.len()).find(|&i| group(i) != current);
    }
    let start = (0..=from)
        .rev()
        .take_while(|&i| group(i) == current)
        .last()
        .unwrap_or(from);
    if start < from {
        return Some(start);
    }
    let previous = start.checked_sub(1)?;
    let before = group(previous);
    Some(
        (0..=previous)
            .rev()
            .take_while(|&i| group(i) == before)
            .last()
            .unwrap_or(previous),
    )
}

/// A console's colour in the system list: stable for a slug, spread around the hue wheel.
pub fn system_tint(slug: &str) -> (u8, u8, u8) {
    let hash = slug.bytes().fold(2_166_136_261_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    });
    let hue = f32::from((hash % 360) as u16);
    let (saturation, lightness) = (0.42_f32, 0.34_f32);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (hue / 60.0) as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    let byte = |v: f32| ((v + m) * 255.0).round() as u8;
    (byte(r), byte(g), byte(b))
}

/// RomM's 0-100 rating as stars out of five in halves, or -1 when the game has none.
pub fn stars(rating: Option<f64>) -> f32 {
    match rating.filter(|r| *r > 0.0) {
        Some(r) => ((r / 20.0).clamp(0.0, 5.0) * 2.0).round() as f32 / 2.0,
        None => -1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_layouts_fall_back_to_the_default() {
        assert_eq!(GameListLayout::by_id("list-detail"), LIST_DETAIL);
        assert_eq!(GameListLayout::by_id("from-the-future"), LIST_DETAIL);
        assert_eq!(GameListLayout::by_id(""), GameListLayout::default());
    }

    #[test]
    fn layout_ids_are_unique() {
        for (i, a) in LAYOUTS.iter().enumerate() {
            assert!(a.columns >= 1);
            assert!(LAYOUTS[i + 1..].iter().all(|b| b.id != a.id));
        }
    }

    #[test]
    fn a_layout_is_saved_as_its_id() {
        let json = serde_json::to_string(&LIST_DETAIL).unwrap();
        assert_eq!(json, r#""list-detail""#);
        let back: GameListLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(back, LIST_DETAIL);
    }

    #[test]
    fn tabs_wrap_in_both_directions() {
        assert_eq!(Tab::stepped(Some(Tab::All), -1), Tab::Recent);
        assert_eq!(Tab::stepped(Some(Tab::Recent), 1), Tab::All);
        assert_eq!(Tab::stepped(Some(Tab::Favorites), 1), Tab::OnDevice);
        assert_eq!(Tab::stepped(None, 1), Tab::All);
        assert_eq!(Tab::stepped(None, -1), Tab::Recent);
        assert_eq!(Tab::from_index(-3), Tab::All);
        assert_eq!(Tab::from_index(9), Tab::All);
        assert_eq!(Tab::from_index(Tab::OnDevice.index()), Tab::OnDevice);
    }

    #[test]
    fn a_tab_is_named_by_what_the_library_is_showing() {
        assert_eq!(Tab::of("all", "c:1", false), Some(Tab::All));
        assert_eq!(Tab::of("all", "c:1", true), Some(Tab::OnDevice));
        assert_eq!(Tab::of("c:1", "c:1", false), Some(Tab::Favorites));
        assert_eq!(Tab::of("recent", "c:1", false), Some(Tab::Recent));
        assert_eq!(Tab::of("p:4", "c:1", false), None);
        assert_eq!(Tab::of("c:2", "c:1", false), None);
    }

    #[test]
    fn leaving_restores_the_window_it_found() {
        let mut windowed = Mode::default();
        assert!(windowed.toggle(false));
        assert!(windowed.active());
        assert!(!windowed.toggle(true));
        assert!(!windowed.active());

        let mut already_fullscreen = Mode::default();
        assert!(already_fullscreen.toggle(true));
        assert!(already_fullscreen.toggle(true));
        assert!(!already_fullscreen.active());
    }
    #[test]
    fn buttons_mean_different_things_on_each_page() {
        use crate::input::{A, L2, X, Y};
        assert_eq!(Action::from_button(Page::Games, A), Some(Action::Accept));
        assert_eq!(Action::from_button(Page::Systems, X), Some(Action::Search));
        assert_eq!(Action::from_button(Page::Games, X), Some(Action::Favorite));
        assert_eq!(Action::from_button(Page::Systems, Y), Some(Action::Random));
        assert_eq!(Action::from_button(Page::Games, Y), Some(Action::Resume));
        assert_eq!(Action::from_button(Page::Game, Y), Some(Action::Download));
        assert_eq!(Action::from_button(Page::Games, L2), Some(Action::JumpBack));
        assert_eq!(Action::from_button(Page::Systems, L2), None);
    }

    #[test]
    fn keys_map_to_actions() {
        use slint::platform::Key;
        let key = |k: Key| char::from(k).to_string();
        assert_eq!(
            Action::from_key(&key(Key::UpArrow), false),
            Some(Action::Up)
        );
        assert_eq!(
            Action::from_key(&key(Key::Escape), false),
            Some(Action::Back)
        );
        assert_eq!(
            Action::from_key(&key(Key::PageDown), false),
            Some(Action::Next)
        );
        assert_eq!(Action::from_key("/", false), Some(Action::Search));
        assert_eq!(Action::from_key("f", true), Some(Action::Search));
        assert_eq!(Action::from_key("f", false), Some(Action::Favorite));
        assert_eq!(Action::from_key("q", false), None);
        assert_eq!(Action::from_key("", false), None);
        assert_eq!(Action::from_key("fm", false), None);
    }

    #[test]
    fn letter_jumps_land_on_the_first_game_of_a_letter() {
        let titles = [
            "1942",
            "007",
            "Advance Wars",
            "Aero",
            "Bomber",
            "Bust",
            "Castlevania",
        ];
        assert_eq!(letter_jump(&titles, 0, true), Some(2));
        assert_eq!(letter_jump(&titles, 2, true), Some(4));
        assert_eq!(letter_jump(&titles, 3, false), Some(2));
        assert_eq!(letter_jump(&titles, 2, false), Some(0));
        assert_eq!(letter_jump(&titles, 0, false), None);
        assert_eq!(letter_jump(&titles, 6, true), None);
        assert_eq!(letter_jump(&titles, 6, false), Some(4));
        assert_eq!(letter_jump::<&str>(&[], 0, true), None);
    }

    #[test]
    fn a_console_keeps_its_colour() {
        assert_eq!(system_tint("gba"), system_tint("gba"));
        assert_ne!(system_tint("gba"), system_tint("snes"));
    }

    #[test]
    fn ratings_become_half_stars() {
        assert_eq!(stars(None), -1.0);
        assert_eq!(stars(Some(0.0)), -1.0);
        assert_eq!(stars(Some(89.0)), 4.5);
        assert_eq!(stars(Some(100.0)), 5.0);
        assert_eq!(stars(Some(250.0)), 5.0);
        assert_eq!(stars(Some(50.0)), 2.5);
    }
}
