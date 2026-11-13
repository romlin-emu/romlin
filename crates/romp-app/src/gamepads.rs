use crate::accessories::{self, buzz};
use crate::input;
use crate::mapping::{self, Mappings, BUTTONS};
use crate::pad_labels::Family;
use romlin_proto::msg::PadState;
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button as SdlButton, GamepadType};
use sdl3::joystick::{Joystick, JoystickId};
use sdl3::{EventPump, GamepadSubsystem, JoystickSubsystem};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ACTIVE_FOR: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadInfo {
    pub key: String,
    pub name: String,
    pub family: Family,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SdlIdentity {
    pub guid: [u8; 16],
    pub path: Option<String>,
}

/// Observed SDL data, before any external emulator's compatibility rules.
#[derive(Clone)]
pub struct SdlController {
    pub identity: SdlIdentity,
    pub mapping: Option<String>,
    pub last_active: Option<Instant>,
    pub player_index: Option<u16>,
}

pub fn pad_keys(uuids: &[[u8; 16]], names: &[&str]) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    uuids
        .iter()
        .zip(names)
        .map(|(uuid, name)| {
            let base = if uuid.iter().all(|b| *b == 0) {
                format!("{:016x}", crate::paths::fnv1a(name.as_bytes()))
            } else {
                uuid.iter().map(|b| format!("{b:02x}")).collect()
            };
            let n = seen.entry(base.clone()).or_default();
            *n += 1;
            format!("{base}#{n}")
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct PadInput {
    pub key: String,
    pub state: PadState,
    /// What menus see, which remaps and the Nintendo swap leave alone.
    pub menu: PadState,
    pub guide: bool,
}

/// A controller's buttons by where they sit. The names are the ones saved in mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    South,
    East,
    West,
    North,
    LeftTrigger,
    RightTrigger,
    LeftTrigger2,
    RightTrigger2,
    LeftThumb,
    RightThumb,
    Start,
    Select,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Mode,
    Unknown,
}

const SDL_BUTTONS: [(Button, SdlButton); 15] = [
    (Button::South, SdlButton::South),
    (Button::East, SdlButton::East),
    (Button::West, SdlButton::West),
    (Button::North, SdlButton::North),
    (Button::LeftTrigger, SdlButton::LeftShoulder),
    (Button::RightTrigger, SdlButton::RightShoulder),
    (Button::LeftThumb, SdlButton::LeftStick),
    (Button::RightThumb, SdlButton::RightStick),
    (Button::Start, SdlButton::Start),
    (Button::Select, SdlButton::Back),
    (Button::DPadUp, SdlButton::DPadUp),
    (Button::DPadDown, SdlButton::DPadDown),
    (Button::DPadLeft, SdlButton::DPadLeft),
    (Button::DPadRight, SdlButton::DPadRight),
    (Button::Mode, SdlButton::Guide),
];

const TRIGGERS: [(Button, Axis); 2] = [
    (Button::LeftTrigger2, Axis::TriggerLeft),
    (Button::RightTrigger2, Axis::TriggerRight),
];

/// How far a trigger goes down before it counts as pressed.
const TRIGGER_PRESS: f32 = 0.75;

fn from_sdl(button: SdlButton) -> Option<Button> {
    SDL_BUTTONS
        .iter()
        .find(|(_, b)| *b == button)
        .map(|(ours, _)| *ours)
}

fn family(kind: GamepadType) -> Family {
    match kind {
        GamepadType::Xbox360 => Family::Xbox360,
        GamepadType::XboxOne => Family::Xbox,
        GamepadType::PS3 => Family::Ps3,
        GamepadType::PS4 => Family::Ps4,
        GamepadType::PS5 => Family::Ps5,
        GamepadType::NintendoSwitchPro
        | GamepadType::NintendoSwitchJoyconLeft
        | GamepadType::NintendoSwitchJoyconRight
        | GamepadType::NintendoSwitchJoyconPair => Family::Nintendo,
        GamepadType::Unknown | GamepadType::Standard => Family::Other,
    }
}

/// The default layout, with A on the button the family confirms with.
fn menu_buttons(pressed: impl Fn(Button) -> bool, family: Family) -> u16 {
    BUTTONS
        .iter()
        .map(|(button, _)| *button)
        .filter(|button| {
            pressed(match *button {
                input::A => family.confirm(),
                input::B => family.back(),
                other => mapping::default_physical(other),
            })
        })
        .fold(0, |buttons, button| buttons | 1 << button)
}

fn axis_value(raw: i16) -> f32 {
    f32::from(raw) / 32767.0
}

/// The id gilrs gave a model, which saved layouts and players are keyed by.
fn model_uuid(bus: u16, vendor: u16, product: u16, version: u16) -> [u8; 16] {
    if vendor == 0 && product == 0 && version == 0 {
        return [0; 16];
    }
    let bus = if cfg!(target_os = "linux") { bus } else { 0x03 };
    let version = if cfg!(windows) { 0 } else { version };
    let mut uuid = [0; 16];
    for (at, value) in [(0, bus), (4, vendor), (8, product), (12, version)] {
        uuid[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    uuid
}

/// Where `Pad::axes` keeps each stick and trigger.
const AXES: [Axis; 6] = [
    Axis::LeftX,
    Axis::LeftY,
    Axis::RightX,
    Axis::RightY,
    Axis::TriggerLeft,
    Axis::TriggerRight,
];

fn bit(button: Button) -> u32 {
    1 << button as u32
}

/// A copy of a controller as the polling thread last saw it.
#[derive(Debug, Clone)]
struct Pad {
    id: u32,
    identity: SdlIdentity,
    uuid: [u8; 16],
    name: String,
    family: Family,
    mapping: Option<String>,
    player_index: Option<u16>,
    buttons: u32,
    axes: [i16; 6],
}

impl Pad {
    fn axis(&self, axis: Axis) -> f32 {
        let at = AXES.iter().position(|a| *a == axis).unwrap_or(0);
        axis_value(self.axes[at])
    }

    fn trigger(&self, axis: Axis) -> f32 {
        self.axis(axis).max(0.0)
    }

    fn value(&self, button: Button) -> f32 {
        if let Some((_, axis)) = TRIGGERS.iter().find(|(b, _)| *b == button) {
            return self.trigger(*axis);
        }
        if self.pressed(button) {
            1.0
        } else {
            0.0
        }
    }

    fn pressed(&self, button: Button) -> bool {
        if let Some((_, axis)) = TRIGGERS.iter().find(|(b, _)| *b == button) {
            return self.trigger(*axis) >= TRIGGER_PRESS;
        }
        self.buttons & bit(button) != 0
    }
}

/// The triggers that went down since `held` was last updated, which they're counted as pressing.
fn trigger_presses(held: &mut [bool; 2], pad: &Pad) -> Vec<Button> {
    let mut pressed = Vec::new();
    for (i, (button, axis)) in TRIGGERS.iter().enumerate() {
        let down = pad.trigger(*axis) >= TRIGGER_PRESS;
        if down && !held[i] {
            pressed.push(*button);
        }
        held[i] = down;
    }
    pressed
}

/// What the polling thread hands over: every controller, and the presses since last asked.
#[derive(Default)]
struct Shared {
    pads: Vec<Pad>,
    buzz: Option<[u8; 4]>,
    presses: Vec<(u32, Button)>,
    /// Each controller's strong and weak motor, and for how long, for the polling thread to set.
    rumble: Option<(Vec<Motors>, Duration)>,
}

/// A controller's id with its strong and weak motor.
type Motors = (u32, u16, u16);

/// How long the polling thread waits for SDL between copies, at most.
const POLL_EVERY: Duration = Duration::from_millis(4);

/// A Buzz! receiver, read as a joystick: its buttons are the handsets' buttons.
struct Receiver {
    id: JoystickId,
    joystick: Joystick,
}

/// SDL lives on its own thread. On macOS, finding controllers runs the calling thread's
/// event loop, and run on the window's thread from inside a window event, that aborts the app.
struct Sdl {
    _context: sdl3::Sdl,
    subsystem: GamepadSubsystem,
    joysticks: JoystickSubsystem,
    events: EventPump,
    open: Vec<(sdl3::gamepad::Gamepad, Pad, [bool; 2])>,
    receivers: Vec<Receiver>,
    /// Controllers that refused to rumble, and those that rumbled, so the log says so once.
    refused: HashSet<u32>,
    rumbled: HashSet<u32>,
}

impl Sdl {
    /// Passive SDL reads controllers as the system reports them and opens no HID device, which
    /// an external emulator then has to itself.
    fn new(passive: bool) -> Result<Self, sdl3::Error> {
        crate::eden::input::configure_sdl();
        sdl3::hint::set("SDL_NO_SIGNAL_HANDLERS", "1");
        sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        sdl3::hint::set("SDL_JOYSTICK_HIDAPI", if passive { "0" } else { "1" });
        let context = sdl3::init()?;
        let subsystem = context.gamepad()?;
        let joysticks = context.joystick()?;
        let events = context.event_pump()?;
        let mut sdl = Self {
            _context: context,
            subsystem,
            joysticks,
            events,
            open: Vec::new(),
            receivers: Vec::new(),
            refused: HashSet::new(),
            rumbled: HashSet::new(),
        };
        for id in sdl.subsystem.gamepads()? {
            sdl.open(id);
        }
        for id in sdl.joysticks.joysticks()? {
            sdl.open_receiver(id);
        }
        Ok(sdl)
    }

    fn is_buzz_receiver(&self, id: JoystickId) -> bool {
        match (
            self.subsystem.vendor_for_id(id),
            self.subsystem.product_for_id(id),
        ) {
            (Some(vendor), Some(product)) => accessories::is_buzz_receiver(vendor, product),
            _ => false,
        }
    }

    fn open_receiver(&mut self, id: JoystickId) {
        if self.receivers.iter().any(|r| r.id == id) || !self.is_buzz_receiver(id) {
            return;
        }
        match self.joysticks.open(id) {
            Ok(joystick) => {
                tracing::info!("Buzz! receiver connected");
                self.receivers.push(Receiver { id, joystick });
            }
            Err(e) => tracing::warn!("couldn't open the Buzz! receiver: {e}"),
        }
    }

    /// Four handsets' buttons, one 5-bit mask each, when a receiver is plugged in.
    fn buzz_handsets(&self) -> Option<[u8; 4]> {
        let receiver = self.receivers.first()?;
        let handset = |h: u32| {
            (0..buzz::BUTTONS).fold(0, |mask, bit| {
                let pressed = receiver
                    .joystick
                    .button(h * buzz::BUTTONS + bit)
                    .unwrap_or(false);
                mask | u8::from(pressed) << bit
            })
        };
        Some(std::array::from_fn(|h| handset(h as u32)))
    }

    fn open(&mut self, id: JoystickId) {
        let raw = u32::from(id);
        if self.open.iter().any(|(_, p, _)| p.id == raw) || self.is_buzz_receiver(id) {
            return;
        }
        let gamepad = match self.subsystem.open(id) {
            Ok(gamepad) => gamepad,
            Err(e) => {
                tracing::warn!("couldn't open a controller: {e}");
                return;
            }
        };
        let guid = self.subsystem.guid_for_id(id).raw().data;
        let uuid = model_uuid(
            u16::from_le_bytes([guid[0], guid[1]]),
            gamepad.vendor_id().unwrap_or(0),
            gamepad.product_id().unwrap_or(0),
            gamepad.product_version().unwrap_or(0),
        );
        let name = gamepad.name().unwrap_or_else(|| "Controller".into());
        let family = family(gamepad.r#type());
        // SAFETY: the gamepad is open, which is all SDL asks of a properties query.
        let rumble = unsafe { gamepad.has_rumble() };
        tracing::info!(rumble, "controller connected: {name}");
        let pad = Pad {
            id: raw,
            identity: SdlIdentity {
                guid,
                path: gamepad.path(),
            },
            uuid,
            name,
            family,
            mapping: gamepad.mapping(),
            player_index: gamepad.player_index(),
            buttons: 0,
            axes: [0; 6],
        };
        self.open.push((gamepad, pad, [false; 2]));
        self.open.sort_by_key(|(_, p, _)| p.id);
    }

    fn rumble(&mut self, motors: &[Motors], duration: Duration) {
        let ms = duration.as_millis().try_into().unwrap_or(u32::MAX);
        for (gamepad, pad, _) in &mut self.open {
            let Some(&(_, strong, weak)) = motors.iter().find(|(id, _, _)| *id == pad.id) else {
                continue;
            };
            match gamepad.set_rumble(strong, weak, ms) {
                Err(e) => {
                    if self.refused.insert(pad.id) {
                        tracing::info!("{} can't rumble: {e}", pad.name);
                    }
                }
                Ok(()) if strong > 0 || weak > 0 => {
                    if self.rumbled.insert(pad.id) {
                        tracing::info!(strong, weak, "rumbling {}", pad.name);
                    }
                }
                Ok(()) => {}
            }
        }
    }

    /// Waits a moment for SDL, then copies every controller and the presses since last time.
    fn step(&mut self, presses: &mut Vec<(u32, Button)>) {
        let first = self.events.wait_event_timeout(POLL_EVERY);
        let events: Vec<Event> = first.into_iter().chain(self.events.poll_iter()).collect();
        for event in events {
            match event {
                Event::GamepadAdded { which, .. } => self.open(which),
                Event::GamepadRemoved { which, .. } => {
                    self.open.retain(|(_, p, _)| p.id != u32::from(which));
                }
                Event::JoyDeviceAdded { which, .. } => self.open_receiver(which),
                Event::JoyDeviceRemoved { which, .. } => self.receivers.retain(|r| r.id != which),
                Event::GamepadButtonDown { which, button, .. } => {
                    if let Some(button) = from_sdl(button) {
                        presses.push((u32::from(which), button));
                    }
                }
                _ => {}
            }
        }
        for (gamepad, pad, held) in &mut self.open {
            pad.buttons = SDL_BUTTONS
                .iter()
                .filter(|(_, sdl)| gamepad.button(*sdl))
                .fold(0, |bits, (ours, _)| bits | bit(*ours));
            pad.axes = AXES.map(|a| gamepad.axis(a));
            pad.player_index = gamepad.player_index();
            presses.extend(trigger_presses(held, pad).into_iter().map(|b| (pad.id, b)));
        }
    }

    /// Hands the controllers and the presses taken by `step` to the app's thread.
    fn publish(&self, shared: &Mutex<Shared>, presses: &mut Vec<(u32, Button)>) {
        let mut shared = shared.lock().unwrap();
        shared.pads = self.open.iter().map(|(_, p, _)| p.clone()).collect();
        shared.buzz = self.buzz_handsets();
        shared.presses.append(presses);
        if shared.presses.len() > 32 {
            let extra = shared.presses.len() - 32;
            shared.presses.drain(..extra);
        }
    }
}

/// Starts SDL on a thread of its own and keeps it running until dropped.
struct Poller {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    passive: bool,
}

impl Poller {
    /// Returns once SDL has started, or `None` when it could not.
    fn start(shared: Arc<Mutex<Shared>>, passive: bool) -> Option<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("gamepads".into())
                .spawn(move || {
                    let mut sdl = match Sdl::new(passive) {
                        Ok(sdl) => sdl,
                        Err(e) => {
                            tracing::warn!("gamepads unavailable: {e}");
                            return;
                        }
                    };
                    let _ = ready_tx.send(());
                    let mut presses = Vec::new();
                    while !stop.load(Ordering::Relaxed) {
                        sdl.step(&mut presses);
                        let rumble = shared.lock().unwrap().rumble.take();
                        if let Some((motors, duration)) = rumble {
                            sdl.rumble(&motors, duration);
                        }
                        sdl.publish(&shared, &mut presses);
                    }
                })
                .map_err(|e| tracing::warn!("gamepads unavailable: {e}"))
                .ok()?
        };
        let poller = Self {
            stop,
            thread: Some(thread),
            passive,
        };
        ready_rx.recv().ok().map(|()| poller)
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct Gamepads {
    pads: Vec<Pad>,
    buzz: Option<[u8; 4]>,
    shared: Arc<Mutex<Shared>>,
    poller: Option<Poller>,
    active: HashMap<SdlIdentity, Instant>,
    presses: Vec<(u32, Button)>,
}

impl Gamepads {
    pub fn new() -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        Self {
            pads: Vec::new(),
            buzz: None,
            poller: Poller::start(shared.clone(), false),
            shared,
            active: HashMap::new(),
            presses: Vec::new(),
        }
    }

    /// Gamepads that never see a controller, for tests that need no SDL.
    #[cfg(all(test, target_os = "linux"))]
    pub fn without_controllers() -> Self {
        Self {
            pads: Vec::new(),
            buzz: None,
            shared: Arc::default(),
            poller: None,
            active: HashMap::new(),
            presses: Vec::new(),
        }
    }

    /// Takes the polling thread's latest copy of the controllers and their presses.
    pub fn poll(&mut self) {
        let presses = {
            let mut shared = self.shared.lock().unwrap();
            self.pads.clone_from(&shared.pads);
            self.buzz = shared.buzz;
            std::mem::take(&mut shared.presses)
        };
        let now = Instant::now();
        for (id, button) in presses {
            if let Some(pad) = self.pads.iter().find(|pad| pad.id == id) {
                self.active.insert(pad.identity.clone(), now);
            }
            self.presses.push((id, button));
        }
        if self.presses.len() > 32 {
            let extra = self.presses.len() - 32;
            self.presses.drain(..extra);
        }
    }

    fn pads(&self) -> &[Pad] {
        &self.pads
    }

    fn keys(&self) -> Vec<String> {
        let pads = self.pads();
        let uuids: Vec<[u8; 16]> = pads.iter().map(|p| p.uuid).collect();
        let names: Vec<&str> = pads.iter().map(|p| p.name.as_str()).collect();
        pad_keys(&uuids, &names)
    }

    pub fn connected(&self) -> Vec<PadInfo> {
        self.keys()
            .into_iter()
            .zip(self.pads())
            .map(|(key, pad)| PadInfo {
                key,
                name: pad.name.clone(),
                family: pad.family,
            })
            .collect()
    }

    pub fn sdl_controllers(&self) -> Vec<SdlController> {
        self.pads
            .iter()
            .map(|pad| SdlController {
                identity: pad.identity.clone(),
                mapping: pad.mapping.clone(),
                last_active: self.active.get(&pad.identity).copied(),
                player_index: pad.player_index,
            })
            .collect()
    }

    /// The Buzz! handsets' buttons, when a receiver is plugged in; see `buzz::pad`.
    pub fn buzz_handsets(&self) -> Option<[u8; 4]> {
        self.buzz
    }

    /// Lets go of every controller while an external emulator runs.
    pub fn suspend(&mut self) {
        self.poller.take();
        *self.shared.lock().unwrap() = Shared::default();
        self.pads.clear();
        self.buzz = None;
        self.presses.clear();
    }

    /// Keeps reading controllers while an external emulator runs, without holding their HID
    /// devices; see `Sdl::new`. Nothing rumbles meanwhile.
    pub fn listen_passively(&mut self) {
        self.suspend();
        self.poller = Poller::start(self.shared.clone(), true);
    }

    /// Takes the controllers back after `suspend` or `listen_passively`.
    pub fn resume(&mut self) {
        if self.poller.as_ref().is_none_or(|poller| poller.passive) {
            self.poller.take();
            self.poller = Poller::start(self.shared.clone(), false);
        }
    }

    pub fn states(&self, mappings: &Mappings, nintendo: bool) -> Vec<PadInput> {
        self.keys()
            .into_iter()
            .zip(self.pads())
            .map(|(key, pad)| {
                let model = mapping::model_of(&key);
                let family = mappings.family(model, pad.family);
                let state = pad_state(pad, mappings, model);
                let menu = mapping::stick_to_dpad(PadState {
                    buttons: menu_buttons(|b| pad.pressed(b), family),
                    axes: state.axes,
                });
                let state = game_state(state, mappings, nintendo, family);
                PadInput {
                    key,
                    state,
                    menu,
                    guide: pad.pressed(Button::Mode),
                }
            })
            .collect()
    }

    /// Rumbles each controller's strong and weak motor with what `motors` gives for its key.
    pub fn rumble(&mut self, motors: impl Fn(&str) -> (u16, u16), duration: Duration) {
        let rumble = self
            .keys()
            .iter()
            .zip(self.pads())
            .map(|(key, pad)| {
                let (strong, weak) = motors(key);
                (pad.id, strong, weak)
            })
            .collect();
        self.shared.lock().unwrap().rumble = Some((rumble, duration));
    }

    /// The family of the controller used last, for naming its buttons.
    pub fn family_in_use(&self, mappings: &Mappings) -> Option<Family> {
        self.connected()
            .into_iter()
            .zip(self.pads())
            .max_by_key(|(_, pad)| self.active.get(&pad.identity))
            .map(|(info, pad)| mappings.family(mapping::model_of(&info.key), pad.family))
    }

    pub fn take_presses(&mut self) -> Vec<(String, Button)> {
        let keys: HashMap<u32, String> =
            self.pads().iter().map(|p| p.id).zip(self.keys()).collect();
        std::mem::take(&mut self.presses)
            .into_iter()
            .filter_map(|(id, button)| Some((keys.get(&id)?.clone(), button)))
            .collect()
    }

    pub fn recently_active(&self) -> Vec<String> {
        let now = Instant::now();
        self.keys()
            .into_iter()
            .zip(self.pads())
            .filter(|(_, pad)| {
                self.active
                    .get(&pad.identity)
                    .is_some_and(|t| now.duration_since(*t) < ACTIVE_FOR)
            })
            .map(|(key, _)| key)
            .collect()
    }
}

/// Nintendo games can take A and B by label on other controllers; Nintendo pads already
/// have them there.
fn game_state(state: PadState, mappings: &Mappings, nintendo: bool, family: Family) -> PadState {
    let state = if mappings.stick_dpad {
        mapping::stick_to_dpad(state)
    } else {
        state
    };
    if nintendo && mappings.nintendo_labels && family != Family::Nintendo {
        mapping::swap_face(state)
    } else {
        state
    }
}

fn pad_state(pad: &Pad, mappings: &Mappings, model: &str) -> PadState {
    let mut state = PadState::default();
    for (button, _) in BUTTONS {
        if pad.pressed(mappings.pad_button(model, button)) {
            state.buttons |= 1 << button;
        }
    }
    let axis = |a: Axis| pad.axis(a);
    let trigger = |b: Button| (pad.value(b) * 32767.0) as i16;
    state.axes = [
        input::stick(axis(Axis::LeftX), false),
        input::stick(axis(Axis::LeftY), false),
        input::stick(axis(Axis::RightX), false),
        input::stick(axis(Axis::RightY), false),
        trigger(mappings.pad_button(model, input::L2)),
        trigger(mappings.pad_button(model, input::R2)),
    ];
    state
}

#[cfg(test)]
mod controller_tests;

// SDL's context, event pump and hints are process-global, including during teardown.
#[cfg(test)]
static SDL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_session_reapplies_hints_when_gamepads_resume() {
        let _serial = SDL_TEST_LOCK.lock().unwrap();
        let hints = [
            ("SDL_JOYSTICK_HIDAPI_XBOX", "0", "1"),
            ("SDL_JOYSTICK_HIDAPI_COMBINE_JOY_CONS", "0", "1"),
            ("SDL_JOYSTICK_HIDAPI_VERTICAL_JOY_CONS", "1", "0"),
            ("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0", "1"),
        ];
        let mut pads = Gamepads::new();
        assert!(
            pads.poller.is_some(),
            "SDL must initialize for the lifecycle test"
        );
        for (hint, expected, _) in hints {
            assert_eq!(sdl3::hint::get(hint).as_deref(), Some(expected), "{hint}");
        }
        pads.suspend();
        assert!(pads.poller.is_none());
        for (hint, _, other) in hints {
            assert!(sdl3::hint::set(hint, other));
        }
        pads.resume();
        assert!(
            pads.poller.is_some(),
            "SDL must reopen after the external session"
        );
        for (hint, expected, _) in hints {
            assert_eq!(sdl3::hint::get(hint).as_deref(), Some(expected), "{hint}");
        }
    }

    #[test]
    fn passive_listening_leaves_hid_devices_until_gamepads_resume() {
        let _serial = SDL_TEST_LOCK.lock().unwrap();
        let hidapi = || sdl3::hint::get("SDL_JOYSTICK_HIDAPI");
        let mut pads = Gamepads::new();
        assert!(
            pads.poller.is_some(),
            "SDL must initialize for the lifecycle test"
        );
        assert_eq!(hidapi().as_deref(), Some("1"));
        pads.listen_passively();
        assert!(pads.poller.as_ref().is_some_and(|p| p.passive));
        assert_eq!(hidapi().as_deref(), Some("0"));
        pads.resume();
        assert!(pads.poller.as_ref().is_some_and(|p| !p.passive));
        assert_eq!(hidapi().as_deref(), Some("1"));
        pads.suspend();
        pads.resume();
        assert_eq!(hidapi().as_deref(), Some("1"));
    }

    #[test]
    fn activity_survives_suspension_and_a_new_instance_id() {
        let identity = SdlIdentity {
            guid: [1; 16],
            path: Some("/dev/input/event7".into()),
        };
        let when = Instant::now();
        let mut pads = Gamepads {
            pads: Vec::new(),
            buzz: None,
            shared: Arc::default(),
            poller: None,
            active: HashMap::from([(identity.clone(), when)]),
            presses: Vec::new(),
        };
        pads.suspend();
        // Reopening constructs the same identity independently of SDL's new instance ID.
        let reopened = SdlIdentity {
            guid: [1; 16],
            path: Some("/dev/input/event7".into()),
        };
        assert_eq!(pads.active.get(&reopened), Some(&when));
        pads.suspend(); // A failed launch must preserve it too.
        assert_eq!(pads.active.get(&identity), Some(&when));
    }

    #[test]
    fn identical_pads_get_distinct_keys_and_unknown_ids_use_the_name() {
        let a = [1u8; 16];
        let b = [2u8; 16];
        let keys = pad_keys(&[a, b, a, [0; 16]], &["Pad", "Other", "Pad", "Mystery"]);
        let hex_a = "01".repeat(16);
        assert_eq!(keys[0], format!("{hex_a}#1"));
        assert_eq!(keys[1], format!("{}#1", "02".repeat(16)));
        assert_eq!(keys[2], format!("{hex_a}#2"));
        assert!(keys[3].ends_with("#1") && keys[3].len() == 18);
    }

    #[test]
    fn models_keep_the_ids_gilrs_gave_them() {
        let uuid = model_uuid(0x05, 0x057e, 0x2009, 0x0001);
        let bus = if cfg!(target_os = "linux") {
            0x05
        } else {
            0x03
        };
        let version = if cfg!(windows) { 0x00 } else { 0x01 };
        assert_eq!(
            uuid,
            [bus, 0, 0, 0, 0x7e, 0x05, 0, 0, 0x09, 0x20, 0, 0, version, 0, 0, 0]
        );
        assert_eq!(model_uuid(0x03, 0, 0, 0), [0; 16]);
    }

    #[test]
    fn menus_take_the_bottom_button_as_a_except_on_nintendo_pads() {
        let bottom = |b: Button| b == Button::South;
        assert_eq!(menu_buttons(bottom, Family::Xbox), 1 << input::A);
        assert_eq!(menu_buttons(bottom, Family::Nintendo), 1 << input::B);
        let top_and_up = |b: Button| matches!(b, Button::North | Button::DPadUp);
        assert_eq!(
            menu_buttons(top_and_up, Family::Ps4),
            1 << input::X | 1 << input::UP
        );
    }

    #[test]
    fn nintendo_label_matching_leaves_nintendo_pads_alone() {
        let mut mappings = Mappings::default();
        mappings.nintendo_labels = true;
        let a = PadState {
            buttons: 1 << input::A,
            axes: [0; 6],
        };
        let b = PadState {
            buttons: 1 << input::B,
            axes: [0; 6],
        };
        assert_eq!(game_state(a, &mappings, true, Family::Xbox), b);
        assert_eq!(game_state(a, &mappings, true, Family::Nintendo), a);
        assert_eq!(game_state(a, &mappings, false, Family::Xbox), a);
    }

    fn copied(buttons: &[Button], left_trigger: i16) -> Pad {
        Pad {
            id: 1,
            identity: SdlIdentity {
                guid: [0; 16],
                path: None,
            },
            uuid: [0; 16],
            name: "Pad".into(),
            family: Family::Xbox,
            mapping: None,
            player_index: None,
            buttons: buttons.iter().fold(0, |bits, b| bits | bit(*b)),
            axes: [0, 0, 0, 0, left_trigger, 0],
        }
    }

    #[test]
    fn a_pad_reads_its_buttons_and_triggers_from_the_copy() {
        let pad = copied(&[Button::South, Button::DPadUp], 30_000);
        assert!(pad.pressed(Button::South) && pad.pressed(Button::DPadUp));
        assert!(!pad.pressed(Button::East));
        assert!(pad.pressed(Button::LeftTrigger2), "most of the way down");
        assert!(!pad.pressed(Button::RightTrigger2));
        assert!((pad.value(Button::LeftTrigger2) - 30_000.0 / 32_767.0).abs() < 1e-6);
        assert_eq!(pad.value(Button::South), 1.0);
        assert!(
            !copied(&[], 20_000).pressed(Button::LeftTrigger2),
            "not far enough"
        );
    }

    #[test]
    fn a_trigger_counts_as_one_press_each_time_it_goes_down() {
        let mut held = [false; 2];
        assert!(trigger_presses(&mut held, &copied(&[], 0)).is_empty());
        assert_eq!(
            trigger_presses(&mut held, &copied(&[], 32_767)),
            [Button::LeftTrigger2]
        );
        assert!(trigger_presses(&mut held, &copied(&[], 32_767)).is_empty());
        assert!(trigger_presses(&mut held, &copied(&[], 0)).is_empty());
        assert_eq!(
            trigger_presses(&mut held, &copied(&[], 32_767)),
            [Button::LeftTrigger2]
        );
    }

    #[test]
    fn the_polling_thread_stops_when_its_owner_goes() {
        let _serial = SDL_TEST_LOCK.lock().unwrap();
        let started = Instant::now();
        let mut pads = Gamepads::new();
        pads.poll();
        drop(pads);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn sdl_buttons_map_to_physical_positions() {
        assert_eq!(from_sdl(SdlButton::South), Some(Button::South));
        assert_eq!(from_sdl(SdlButton::Back), Some(Button::Select));
        assert_eq!(from_sdl(SdlButton::LeftShoulder), Some(Button::LeftTrigger));
        assert_eq!(from_sdl(SdlButton::Guide), Some(Button::Mode));
        assert_eq!(from_sdl(SdlButton::Touchpad), None);
    }
}
