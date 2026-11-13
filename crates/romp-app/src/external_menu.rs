//! Romlin's menu over an external emulator's game. Select+Start freezes the emulator and shows
//! Resume and Quit game in a window of Romlin's own above it; nothing goes into the emulator.

use crate::external::Running;
use crate::gamepads::{Button, Gamepads, PadInput};
use crate::input;
use crate::mapping::Mappings;
use crate::pad_labels::Family;
use crate::ExternalMenuWindow;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

/// Where the menu's window can stay above an emulator in full screen: under gamescope, as its
/// external overlay, and on Windows. Elsewhere Select+Start stays with the emulator.
pub fn supported(game_mode: bool) -> bool {
    cfg!(windows) || (cfg!(target_os = "linux") && game_mode)
}

const fn bit(button: u32) -> u16 {
    1 << button
}

const CHORD: u16 = bit(input::SELECT) | bit(input::START);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Open,
    Resume,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Row {
    #[default]
    Resume,
    Quit,
}

/// What the controllers ask of the menu, from the buttons every pad's menus see.
#[derive(Debug, Default)]
struct State {
    open: bool,
    focus: Row,
    chord: bool,
    buttons: u16,
    /// An action waiting for the buttons that asked for it to be let go.
    pending: Option<(Action, u16)>,
}

impl State {
    fn step(&mut self, inputs: &[PadInput]) -> Option<Action> {
        let buttons = inputs.iter().fold(0, |b, p| b | p.menu.buttons);
        let pressed = buttons & !std::mem::replace(&mut self.buttons, buttons);
        let chord = inputs.iter().any(|p| p.menu.buttons & CHORD == CHORD);
        let chord_pressed = chord && !self.chord;
        self.chord = chord;
        if let Some((action, held)) = self.pending {
            if buttons & held != 0 {
                return None;
            }
            self.dismiss();
            return Some(action);
        }
        if !self.open {
            if chord_pressed {
                self.open = true;
                self.focus = Row::Resume;
                return Some(Action::Open);
            }
            return None;
        }
        if chord_pressed {
            return self.resume_after(CHORD);
        }
        if pressed & bit(input::UP) != 0 {
            self.focus = Row::Resume;
        }
        if pressed & bit(input::DOWN) != 0 {
            self.focus = Row::Quit;
        }
        if pressed & bit(input::A) != 0 {
            return match self.focus {
                Row::Resume => self.resume_after(bit(input::A)),
                Row::Quit => Some(Action::Quit),
            };
        }
        if pressed & bit(input::B) != 0 {
            return self.resume_after(bit(input::B));
        }
        None
    }

    /// Resumes once `buttons` are let go, so the game never sees the press that closed the menu.
    /// Only those buttons count, so a drifting stick can't hold the game paused.
    fn resume_after(&mut self, buttons: u16) -> Option<Action> {
        self.pending = Some((Action::Resume, buttons));
        None
    }

    fn dismiss(&mut self) {
        self.open = false;
        self.pending = None;
    }
}

/// The menu's window, and on Linux the gamescope overlay it becomes.
struct Surface {
    window: ExternalMenuWindow,
    /// Whether the native window exists and has been set up.
    settled: Cell<bool>,
    #[cfg(target_os = "linux")]
    overlay: RefCell<Option<Result<gamescope::Overlay, String>>>,
}

impl Surface {
    fn new(title: &str) -> Result<Self, String> {
        let window = ExternalMenuWindow::new().map_err(|e| e.to_string())?;
        window.set_name(title.into());
        let surface = Self {
            window,
            settled: Cell::new(false),
            #[cfg(target_os = "linux")]
            overlay: RefCell::new(None),
        };
        // Under gamescope the window stays mapped for the whole game, and opening only changes its
        // opacity: a window mapped later would count as an app window and take focus for a frame.
        #[cfg(target_os = "linux")]
        surface.window.show().map_err(|e| e.to_string())?;
        Ok(surface)
    }

    /// Sets up the native window once Slint has made it, which happens after `show` returns.
    fn settle(&self) {
        if self.settled.get() || self.window.window().with_winit_window(|_| ()).is_none() {
            return;
        }
        self.settled.set(true);
        crate::scale::track(&self.window);
        #[cfg(target_os = "linux")]
        {
            fit_to_monitor(self.window.window());
            let overlay = gamescope::Overlay::attach(self.window.window());
            if let Err(e) = &overlay {
                tracing::warn!("the game menu can't show over the game: {e}");
            }
            *self.overlay.borrow_mut() = Some(overlay);
        }
    }

    #[cfg(target_os = "linux")]
    fn show(&self, on: bool) -> Result<(), String> {
        let overlay = self.overlay.borrow();
        let overlay = match overlay.as_ref() {
            Some(Ok(overlay)) => overlay,
            Some(Err(e)) => return Err(e.clone()),
            None => return Err("the menu's window isn't ready yet".into()),
        };
        if on {
            self.window.set_shown(true);
            overlay.show(true)
        } else {
            let hidden = overlay.show(false);
            self.window.set_shown(false);
            hidden
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn show(&self, on: bool) -> Result<(), String> {
        self.window.set_shown(on);
        if on {
            #[cfg(windows)]
            fit_to_primary_monitor(self.window.window());
            self.window.show().map_err(|e| e.to_string())
        } else {
            self.window.hide().map_err(|e| e.to_string())
        }
    }
}

/// Gamescope draws an overlay from the output's top-left, unscaled, so it covers the output.
#[cfg(target_os = "linux")]
fn fit_to_monitor(window: &slint::Window) {
    let size = window
        .with_winit_window(|w| {
            w.current_monitor()
                .or_else(|| w.primary_monitor())
                .map(|m| m.size())
        })
        .flatten();
    if let Some(size) = size {
        window.set_position(slint::PhysicalPosition::new(0, 0));
        window.set_size(slint::PhysicalSize::new(size.width, size.height));
    }
}

#[cfg(windows)]
fn fit_to_primary_monitor(window: &slint::Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    // SAFETY: GetSystemMetrics only reads a system value.
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    if let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) {
        if width > 0 && height > 0 {
            window.set_position(slint::PhysicalPosition::new(0, 0));
            window.set_size(slint::PhysicalSize::new(width, height));
        }
    }
}

#[cfg(target_os = "linux")]
mod gamescope {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::WinitWindowAccessor;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    /// Gamescope composites a window with this set above the focused game, and never focuses it.
    pub const OVERLAY: &[u8] = b"GAMESCOPE_EXTERNAL_OVERLAY";
    /// Gamescope doesn't paint an overlay at opacity 0.
    pub const OPACITY: &[u8] = b"_NET_WM_WINDOW_OPACITY";

    /// The menu's X11 window, tagged as gamescope's external overlay over a connection of its own.
    pub struct Overlay {
        connection: RustConnection,
        pub window: u32,
        opacity: u32,
    }

    pub fn atom(connection: &impl Connection, name: &[u8]) -> Result<u32, String> {
        Ok(connection
            .intern_atom(false, name)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .atom)
    }

    impl Overlay {
        pub fn attach(window: &slint::Window) -> Result<Self, String> {
            let window = window
                .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
                    RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
                    RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
                    _ => None,
                })
                .flatten()
                .ok_or("its window isn't an X11 window")?;
            let (connection, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
            let overlay = atom(&connection, OVERLAY)?;
            let opacity = atom(&connection, OPACITY)?;
            let attached = Self {
                connection,
                window,
                opacity,
            };
            attached.show(false)?;
            attached.set(overlay, 1)?;
            Ok(attached)
        }

        fn set(&self, property: u32, value: u32) -> Result<(), String> {
            self.connection
                .change_property32(
                    PropMode::REPLACE,
                    self.window,
                    property,
                    AtomEnum::CARDINAL,
                    &[value],
                )
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())
        }

        pub fn show(&self, on: bool) -> Result<(), String> {
            self.set(self.opacity, if on { u32::MAX } else { 0 })
        }
    }
}

/// The menu for one external game, from launch until the game closes.
pub struct Menu {
    inner: Rc<Inner>,
    _timer: Timer,
}

struct Inner {
    running: Running,
    gamepads: Rc<RefCell<Gamepads>>,
    mappings: Rc<RefCell<Mappings>>,
    state: RefCell<State>,
    surface: Option<Surface>,
    family: Cell<Option<Family>>,
    frozen: Cell<bool>,
    closing: Cell<bool>,
}

impl Menu {
    pub fn start(
        running: Running,
        gamepads: Rc<RefCell<Gamepads>>,
        mappings: Rc<RefCell<Mappings>>,
        title: &str,
    ) -> Self {
        let surface = Surface::new(title)
            .map_err(|e| tracing::warn!("the game menu has no window, so its chord quits: {e}"))
            .ok();
        let inner = Rc::new(Inner {
            running,
            gamepads,
            mappings,
            state: RefCell::default(),
            surface,
            family: Cell::new(None),
            frozen: Cell::new(false),
            closing: Cell::new(false),
        });
        if let Some(surface) = &inner.surface {
            let window = &surface.window;
            let weak = Rc::downgrade(&inner);
            window.on_resume(move || with(&weak, Inner::resume));
            let weak = Rc::downgrade(&inner);
            window.on_quit(move || with(&weak, Inner::quit));
            let weak = Rc::downgrade(&inner);
            window.window().on_close_requested(move || {
                with(&weak, Inner::resume);
                slint::CloseRequestResponse::KeepWindowShown
            });
        }
        let timer = Timer::default();
        let weak = Rc::downgrade(&inner);
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            with(&weak, Inner::tick);
        });
        inner.tick();
        Self {
            inner,
            _timer: timer,
        }
    }
}

fn with(inner: &Weak<Inner>, f: impl FnOnce(&Inner)) {
    if let Some(inner) = inner.upgrade() {
        f(&inner);
    }
}

impl Inner {
    fn tick(&self) {
        if let Some(surface) = &self.surface {
            surface.settle();
        }
        if self.closing.get() {
            return;
        }
        let (inputs, family) = {
            let Ok(mut gamepads) = self.gamepads.try_borrow_mut() else {
                return;
            };
            gamepads.poll();
            let mappings = self.mappings.borrow();
            (
                gamepads.states(&mappings, false),
                gamepads.family_in_use(&mappings),
            )
        };
        self.name_chord(family.unwrap_or_default());
        let action = self.state.borrow_mut().step(&inputs);
        if let Some(surface) = &self.surface {
            let focus = self.state.borrow().focus;
            surface.window.set_focused_item(focus as i32);
        }
        match action {
            Some(Action::Open) => self.open(),
            Some(Action::Resume) => self.resume(),
            Some(Action::Quit) => self.quit(),
            None => {}
        }
    }

    fn name_chord(&self, family: Family) {
        if self.family.replace(Some(family)) == Some(family) {
            return;
        }
        let name = |button| crate::i18n::translate(crate::pad_labels::name(family, button));
        if let Some(surface) = &self.surface {
            let chord = format!("{}+{}", name(Button::Select), name(Button::Start));
            surface.window.set_chord_name(chord.into());
        }
    }

    fn open(&self) {
        let opened = self
            .running
            .freeze(true)
            .map_err(|e| format!("couldn't pause the emulator: {e}"))
            .and_then(|()| {
                self.frozen.set(true);
                self.surface
                    .as_ref()
                    .ok_or_else(|| "it has no window".to_string())?
                    .show(true)
            });
        if let Err(e) = opened {
            tracing::warn!("the game menu didn't open, so the game closes: {e}");
            self.quit();
        }
    }

    fn resume(&self) {
        if self.closing.get() {
            return;
        }
        self.state.borrow_mut().dismiss();
        if let Some(surface) = &self.surface {
            if let Err(e) = surface.show(false) {
                tracing::warn!("hiding the game menu: {e}");
            }
        }
        match self.running.freeze(false) {
            Ok(()) => self.frozen.set(false),
            Err(e) => {
                tracing::warn!("the emulator didn't continue, so the game closes: {e}");
                self.quit();
            }
        }
    }

    fn quit(&self) {
        if self.closing.replace(true) {
            return;
        }
        if let Some(surface) = &self.surface {
            surface.window.set_closing(true);
        }
        // Continues the emulator before asking it to exit.
        self.running.close();
        self.frozen.set(false);
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        if let Some(surface) = &self.inner.surface {
            let _ = surface.window.hide();
        }
        if self.inner.frozen.get() {
            if let Err(e) = self.inner.running.freeze(false) {
                tracing::warn!("continuing the emulator: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use romlin_proto::msg::PadState;

    fn pad(buttons: &[u32]) -> PadInput {
        PadInput {
            key: "pad#1".into(),
            state: PadState::default(),
            menu: PadState {
                buttons: buttons.iter().fold(0, |b, button| b | bit(*button)),
                axes: [0; 6],
            },
            guide: false,
        }
    }

    /// Feeds one frame per entry, all from one pad, and collects what each frame asked for.
    fn run(state: &mut State, frames: &[&[u32]]) -> Vec<Option<Action>> {
        frames.iter().map(|b| state.step(&[pad(b)])).collect()
    }

    const SELECT: u32 = input::SELECT;
    const START: u32 = input::START;

    #[test]
    fn the_chord_opens_the_menu_at_once() {
        let mut state = State::default();
        assert_eq!(
            run(
                &mut state,
                &[&[], &[SELECT], &[SELECT, START], &[SELECT, START], &[]]
            ),
            [None, None, Some(Action::Open), None, None]
        );
        assert!(state.open);
    }

    #[test]
    fn the_chord_on_any_one_pad_opens_it_but_not_across_pads() {
        let mut state = State::default();
        assert_eq!(state.step(&[pad(&[SELECT]), pad(&[START])]), None);
        assert_eq!(
            state.step(&[pad(&[]), pad(&[SELECT, START])]),
            Some(Action::Open)
        );
    }

    #[test]
    fn a_second_chord_resumes_only_once_it_is_let_go() {
        let mut state = State::default();
        let frames: &[&[u32]] = &[
            &[SELECT, START],
            &[],
            &[SELECT, START],
            &[SELECT, START],
            &[START],
            &[],
        ];
        assert_eq!(
            run(&mut state, frames),
            [
                Some(Action::Open),
                None,
                None,
                None,
                None,
                Some(Action::Resume)
            ]
        );
        assert!(!state.open);
    }

    #[test]
    fn b_resumes_once_it_is_let_go() {
        let mut state = State::default();
        let frames: &[&[u32]] = &[&[SELECT, START], &[], &[input::B], &[input::B], &[]];
        assert_eq!(
            run(&mut state, frames),
            [Some(Action::Open), None, None, None, Some(Action::Resume)]
        );
    }

    #[test]
    fn down_then_a_quits_at_once() {
        let mut state = State::default();
        let frames: &[&[u32]] = &[&[SELECT, START], &[], &[input::DOWN], &[], &[input::A]];
        assert_eq!(
            run(&mut state, frames),
            [Some(Action::Open), None, None, None, Some(Action::Quit)]
        );
        assert_eq!(state.focus, Row::Quit);
    }

    #[test]
    fn a_on_resume_resumes_and_reopening_starts_on_resume() {
        let mut state = State::default();
        let frames: &[&[u32]] = &[
            &[SELECT, START],
            &[],
            &[input::DOWN],
            &[input::UP],
            &[input::A],
            &[],
        ];
        assert_eq!(
            run(&mut state, frames),
            [
                Some(Action::Open),
                None,
                None,
                None,
                None,
                Some(Action::Resume)
            ]
        );
        state.step(&[pad(&[input::DOWN])]);
        state.step(&[pad(&[])]);
        assert_eq!(state.step(&[pad(&[SELECT, START])]), Some(Action::Open));
        assert_eq!(state.focus, Row::Resume);
    }

    #[test]
    fn a_direction_held_throughout_does_not_block_resume() {
        let mut state = State::default();
        let left = input::LEFT;
        let frames: &[&[u32]] = &[&[left, SELECT, START], &[left], &[left, input::B], &[left]];
        assert_eq!(
            run(&mut state, frames),
            [Some(Action::Open), None, None, Some(Action::Resume)]
        );
    }

    #[test]
    fn start_alone_does_not_activate() {
        let mut state = State::default();
        let frames: &[&[u32]] = &[&[SELECT, START], &[], &[START], &[], &[START]];
        assert_eq!(
            run(&mut state, frames),
            [Some(Action::Open), None, None, None, None]
        );
        assert!(state.open);
        assert!(state.pending.is_none());
    }

    #[test]
    fn closed_menus_ignore_menu_buttons() {
        let mut state = State::default();
        let frames: &[&[u32]] = &[&[input::A], &[input::B], &[START], &[input::DOWN]];
        assert_eq!(run(&mut state, frames), [None, None, None, None]);
        assert!(!state.open);
    }

    #[test]
    fn the_menu_is_offered_only_where_its_window_can_show() {
        assert_eq!(supported(true), cfg!(windows) || cfg!(target_os = "linux"));
        assert_eq!(supported(false), cfg!(windows));
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires X11: xvfb-run -a cargo test -p romlin-app gamescope_overlay_lifecycle -- --ignored"]
    fn gamescope_overlay_lifecycle() {
        use slint::winit_030::winit::platform::x11::EventLoopBuilderExtX11;
        use x11rb::connection::Connection as _;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

        let mut builder = slint::winit_030::winit::event_loop::EventLoop::with_user_event();
        builder.with_x11().with_any_thread(true);
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("software".into())
            .with_winit_event_loop_builder(builder)
            .select()
            .unwrap();
        let mut sleep = std::process::Command::new("sleep");
        sleep.arg("30");
        let running = crate::external::launch(sleep, "sleep", |_, _| {}).unwrap();
        let menu = Menu::start(
            running.clone(),
            Rc::new(RefCell::new(Gamepads::without_controllers())),
            Rc::default(),
            "Game",
        );
        let (x11, _) = x11rb::connect(None).unwrap();
        let read = move |window: u32, name: &[u8]| {
            let atom = gamescope::atom(&x11, name).unwrap();
            let reply = x11
                .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1)
                .unwrap()
                .reply()
                .unwrap();
            let value = reply.value32().and_then(|mut v| v.next());
            x11.flush().unwrap();
            value
        };
        let checks = slint::Timer::default();
        let started = std::time::Instant::now();
        let done = Cell::new(false);
        checks.start(TimerMode::Repeated, Duration::from_millis(20), move || {
            if done.get() {
                return;
            }
            let surface = menu.inner.surface.as_ref().expect("the menu has a window");
            let window = match surface.overlay.borrow().as_ref() {
                Some(Ok(overlay)) => overlay.window,
                Some(Err(e)) => panic!("{e}"),
                None => {
                    assert!(started.elapsed() < Duration::from_secs(10), "no window");
                    return;
                }
            };
            assert_eq!(read(window, gamescope::OVERLAY), Some(1));
            assert_eq!(read(window, gamescope::OPACITY), Some(0));
            menu.inner.open();
            assert!(menu.inner.frozen.get());
            assert!(surface.window.get_shown());
            assert_eq!(read(window, gamescope::OPACITY), Some(u32::MAX));
            menu.inner.resume();
            assert!(!menu.inner.frozen.get());
            assert!(!surface.window.get_shown());
            assert_eq!(read(window, gamescope::OPACITY), Some(0));
            assert_eq!(read(window, gamescope::OVERLAY), Some(1));
            done.set(true);
            slint::quit_event_loop().unwrap();
        });
        slint::run_event_loop_until_quit().unwrap();
        drop(checks);
        running.close();
        assert!(running.wait_exit(Duration::from_secs(10)));
    }
}
