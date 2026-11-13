use super::*;
use crate::input::{Command, Controls};
use anyhow::{ensure, Context};
use romlin_proto::msg::AppMsg;

/// Polls until `condition` holds. `pump` stands in for the polling thread when the test owns SDL.
fn wait_with(
    pads: &mut Gamepads,
    mut pump: impl FnMut(),
    condition: impl Fn(&Gamepads) -> bool,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        pump();
        pads.poll();
        if condition(pads) {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "timed out waiting for SDL input");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(all(windows, target_arch = "x86_64", feature = "controller-hidmaestro"))]
fn wait_for(pads: &mut Gamepads, condition: impl Fn(&Gamepads) -> bool) -> anyhow::Result<()> {
    wait_with(pads, || {}, condition)
}

fn state(pads: &Gamepads, key: &str, mappings: &Mappings) -> PadInput {
    pads.states(mappings, false)
        .into_iter()
        .find(|p| p.key == key)
        .expect("test controller is connected")
}

#[test]
fn sdl_virtual_controller_reaches_game_input() -> anyhow::Result<()> {
    use sdl3::joystick::{JoystickType, VirtualJoystickDescription};

    let _serial = SDL_TEST_LOCK.lock().unwrap();
    // This test owns SDL, which is process-wide, so it does the polling thread's work itself.
    let mut sdl = Sdl::new(false).context("SDL initialization failed")?;
    let mut pads = Gamepads {
        pads: Vec::new(),
        buzz: None,
        shared: Arc::default(),
        poller: None,
        active: HashMap::new(),
        presses: Vec::new(),
    };
    let shared = pads.shared.clone();
    let joysticks = sdl.joysticks.clone();
    let mut pump = || {
        let mut presses = Vec::new();
        sdl.step(&mut presses);
        sdl.publish(&shared, &mut presses);
    };
    let connection = joysticks.attach_virtual_joystick(
        VirtualJoystickDescription::new()
            .name("Romlin controller test")
            .joystick_type(JoystickType::Gamepad)
            .with_buttons(SDL_BUTTONS.iter().map(|(_, b)| *b))
            .with_axes([
                Axis::LeftX,
                Axis::LeftY,
                Axis::RightX,
                Axis::RightY,
                Axis::TriggerLeft,
                Axis::TriggerRight,
            ]),
    )?;
    let joystick = joysticks.open(connection.id())?;
    // SDL virtual triggers use the full signed range, with -32768 released.
    joystick.set_virtual_axis(Axis::TriggerLeft as u32, i16::MIN)?;
    joystick.set_virtual_axis(Axis::TriggerRight as u32, i16::MIN)?;
    wait_with(&mut pads, &mut pump, |p| {
        p.pads().iter().any(|p| p.id == u32::from(connection.id()))
    })?;
    let key = pads.keys()[pads
        .pads()
        .iter()
        .position(|p| p.id == u32::from(connection.id()))
        .unwrap()]
    .clone();
    let mappings = Mappings::default();
    assert_eq!(state(&pads, &key, &mappings).state, PadState::default());

    joystick.set_virtual_axis(Axis::TriggerLeft as u32, 0)?;
    wait_with(&mut pads, &mut pump, |p| {
        state(p, &key, &mappings).state.axes[4] > 16000
    })?;
    assert_eq!(state(&pads, &key, &mappings).state.buttons, 0);
    assert!(pads.take_presses().iter().all(|(k, _)| k != &key));

    joystick.set_virtual_button(SdlButton::South as u32, true)?;
    joystick.set_virtual_axis(Axis::LeftX as u32, 24000)?;
    joystick.set_virtual_axis(Axis::RightY as u32, -24000)?;
    joystick.set_virtual_axis(Axis::TriggerLeft as u32, i16::MAX)?;
    wait_with(&mut pads, &mut pump, |p| {
        state(p, &key, &mappings).state.buttons == (1 << input::B | 1 << input::L2)
    })?;
    let pressed = state(&pads, &key, &mappings);
    assert_eq!(pressed.state.axes, [24000, 0, 0, -24000, 32767, 0]);
    assert_eq!(
        pressed.menu.buttons & (1 << input::A | 1 << input::RIGHT),
        1 << input::A | 1 << input::RIGHT
    );
    let presses = pads.take_presses();
    assert!(presses.contains(&(key.clone(), Button::South)));
    assert!(presses.contains(&(key.clone(), Button::LeftTrigger2)));
    assert!(pads.recently_active().contains(&key));

    let mut remapped = Mappings::default();
    remapped.set_pad_button(mapping::model_of(&key), input::A, Button::South);
    let remapped = state(&pads, &key, &remapped);
    assert_ne!(remapped.state.buttons & (1 << input::A), 0);
    assert_eq!(remapped.menu, pressed.menu);

    let mut players = crate::players::Assignments::default();
    assert_eq!(players.connect(&key, &[]), Some(1));
    let mut controls = Controls::default();
    assert_eq!(
        controls.set_gamepads(vec![(players.player(&key), pressed.state)]),
        [Command::Send(AppMsg::Pad {
            port: 0,
            state: pressed.state
        })]
    );
    assert!(controls
        .set_gamepads(vec![(Some(1), pressed.state)])
        .is_empty());

    joystick.set_virtual_button(SdlButton::South as u32, false)?;
    joystick.set_virtual_axis(Axis::LeftX as u32, 1000)?;
    joystick.set_virtual_axis(Axis::RightY as u32, 0)?;
    joystick.set_virtual_axis(Axis::TriggerLeft as u32, i16::MIN)?;
    wait_with(&mut pads, &mut pump, |p| {
        state(p, &key, &mappings).state == PadState::default()
    })?;
    assert_eq!(
        controls.set_gamepads(vec![(Some(1), state(&pads, &key, &mappings).state)]),
        [Command::Send(AppMsg::Pad {
            port: 0,
            state: PadState::default()
        })]
    );

    // Unplug while held must also release the runner's input.
    joystick.set_virtual_button(SdlButton::South as u32, true)?;
    wait_with(&mut pads, &mut pump, |p| {
        state(p, &key, &mappings).state.buttons == 1 << input::B
    })?;
    controls.set_gamepads(vec![(Some(1), state(&pads, &key, &mappings).state)]);
    drop(joystick);
    drop(connection);
    wait_with(&mut pads, &mut pump, |p| {
        !p.connected().iter().any(|p| p.key == key)
    })?;
    assert_eq!(
        controls.set_gamepads(vec![]),
        [Command::Send(AppMsg::Pad {
            port: 0,
            state: PadState::default()
        })]
    );
    Ok(())
}

#[cfg(all(windows, target_arch = "x86_64", feature = "controller-hidmaestro"))]
#[test]
#[ignore = "requires an elevated hidmaestro bridge and installed driver; see docs/controller-testing.md"]
fn hidmaestro_controller_reaches_sdl() -> anyhow::Result<()> {
    use hidmaestro::{Buttons, HidMaestro, StandardAxes};

    let _serial = SDL_TEST_LOCK.lock().unwrap();
    let pipe = std::env::var("HIDMAESTRO_PIPE_NAME")
        .context("set HIDMAESTRO_PIPE_NAME to an already-running dedicated broker")?;
    ensure!(!pipe.is_empty(), "HIDMAESTRO_PIPE_NAME must not be empty");
    let mut hm = HidMaestro::builder()
        .pipe_name(pipe)
        .response_timeout(Duration::from_secs(15))
        .spawn()?;
    ensure!(
        hm.is_driver_installed()?,
        "install the driver separately before testing"
    );
    hm.load_default_profiles()?;
    let mut pads = Gamepads::new();
    ensure!(pads.poller.is_some(), "SDL initialization failed");
    let before = pads.keys();
    let controller = hm.create_controller("xbox-360-wired", None)?;
    let result = (|| -> anyhow::Result<()> {
        wait_for(&mut pads, |p| p.keys().iter().any(|k| !before.contains(k)))?;
        let added: Vec<_> = pads
            .connected()
            .into_iter()
            .filter(|p| !before.contains(&p.key))
            .collect();
        ensure!(
            added.len() == 1,
            "expected exactly one newly connected controller"
        );
        let key = &added[0].key;
        ensure!(
            matches!(added[0].family, Family::Xbox360 | Family::Xbox),
            "expected Xbox profile"
        );
        let mappings = Mappings::default();
        let observed = |p: &Gamepads| {
            p.states(&mappings, false)
                .into_iter()
                .find(|p| p.key == *key)
        };
        hm.controller(&controller)?.reset()?;
        wait_for(&mut pads, |p| {
            observed(p).is_some_and(|p| p.state == PadState::default())
        })?;
        hm.controller(&controller)?.press(Buttons::A)?;
        hm.controller(&controller)?
            .set_standard_axes(StandardAxes {
                left_stick_x: Some(1.0),
                left_trigger: Some(1.0),
                ..Default::default()
            })?;
        wait_for(&mut pads, |p| {
            observed(p).is_some_and(|input| {
                input.state.buttons == (1 << input::B | 1 << input::L2)
                    && input.state.axes[0] > 30000
                    && input.state.axes[4] > 30000
                    && input.menu.buttons & (1 << input::A) != 0
            })
        })?;
        hm.controller(&controller)?.reset()?;
        wait_for(&mut pads, |p| {
            observed(p).is_some_and(|p| p.state == PadState::default())
        })?;
        Ok(())
    })();
    // Attempt both operations even if input observation failed.
    let reset = hm.controller(&controller).and_then(|mut p| p.reset());
    let remove = hm.remove_controller(&controller);
    hm.shutdown();
    reset?;
    remove?;
    result?;
    wait_for(&mut pads, |p| p.keys() == before)?;
    Ok(())
}
