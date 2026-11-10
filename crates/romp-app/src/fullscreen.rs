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
}
