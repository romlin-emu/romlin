use crate::gamepads::SdlController;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

type Settings = BTreeMap<String, String>;
const MAX_PLAYERS: usize = 8;

pub struct Prepared {
    pub missing: bool,
    pub steam_virtual: bool,
}

/// Reapply before every SDL initialization; SDL_Quit clears the hints.
pub fn configure_sdl() {
    for (hint, value) in [
        ("SDL_JOYSTICK_HIDAPI_XBOX", "0"),
        ("SDL_JOYSTICK_HIDAPI_COMBINE_JOY_CONS", "0"),
        ("SDL_JOYSTICK_HIDAPI_VERTICAL_JOY_CONS", "1"),
        ("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0"),
    ] {
        sdl3::hint::set(hint, value);
    }
}

/// Refresh all controller slots on every launch. With Romlin's own `menu`, Select+Start opens
/// it instead of closing Eden.
pub fn prepare(exe: &Path, controllers: &[SdlController], menu: bool) -> io::Result<Prepared> {
    let config = super::user_dir(exe).join("config/qt-config.ini");
    let existing = match std::fs::read_to_string(&config) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let (text, prepared) = config_text(&existing, controllers, menu);
    crate::saves::replace_file(&config, text.as_bytes())?;
    Ok(prepared)
}

// SDL2 HIDAPI uses standardized raw button indices; SDL3 moved the D-pad to a hat.
// Verified against SDL2's HIDAPI drivers, not SDL3's generated mapping strings.
const SDL2_STANDARD_HIDAPI: &str = "guid,SDL2 HIDAPI,\
    a:b0,b:b1,x:b2,y:b3,back:b4,guide:b5,start:b6,\
    leftstick:b7,rightstick:b8,leftshoulder:b9,rightshoulder:b10,\
    dpup:b11,dpdown:b12,dpleft:b13,dpright:b14,misc1:b15,\
    leftx:a0,lefty:a1,rightx:a2,righty:a3,lefttrigger:a4,righttrigger:a5,";
const SDL2_JOYCON_LEFT: &str = "guid,SDL2 Joy-Con Left,\
    back:b4,leftshoulder:b9,leftstick:b7,lefttrigger:a4,\
    dpup:b11,dpdown:b12,dpleft:b13,dpright:b14,misc1:b15,leftx:a0,lefty:a1,";
const SDL2_JOYCON_RIGHT: &str = "guid,SDL2 Joy-Con Right,\
    a:b0,b:b1,x:b2,y:b3,guide:b5,start:b6,\
    rightshoulder:b10,rightstick:b8,righttrigger:a5,rightx:a2,righty:a3,";

// Only original devices with verified SDL2 drivers; unknown HIDAPI layouts are unusable.
const HIDAPI_LAYOUTS: &[(u16, u16, &str)] = &[
    (0x28de, 0x1205, SDL2_STANDARD_HIDAPI), // Steam Deck
    (0x054c, 0x05c4, SDL2_STANDARD_HIDAPI), // DualShock 4
    (0x054c, 0x09cc, SDL2_STANDARD_HIDAPI),
    (0x054c, 0x0ba0, SDL2_STANDARD_HIDAPI), // Sony wireless adapter
    (0x054c, 0x0ce6, SDL2_STANDARD_HIDAPI), // DualSense
    (0x054c, 0x0df2, SDL2_STANDARD_HIDAPI), // DualSense Edge
    (0x057e, 0x2009, SDL2_STANDARD_HIDAPI), // Switch Pro
    (0x057e, 0x2006, SDL2_JOYCON_LEFT),
    (0x057e, 0x2007, SDL2_JOYCON_RIGHT),
    (0x1949, 0x0419, SDL2_STANDARD_HIDAPI), // Amazon Luna
    (0x18d1, 0x9400, SDL2_STANDARD_HIDAPI), // Stadia
    (0x0955, 0x7210, SDL2_STANDARD_HIDAPI), // Shield
    (0x0955, 0x7214, SDL2_STANDARD_HIDAPI),
];

fn ids(guid: [u8; 16]) -> (u16, u16) {
    (
        u16::from_le_bytes([guid[4], guid[5]]),
        u16::from_le_bytes([guid[8], guid[9]]),
    )
}

fn sdl2_mapping(controller: &SdlController) -> Option<&str> {
    let mapping = controller.mapping.as_deref()?;
    let guid = controller.identity.guid;
    if guid[14] == b'h' {
        let (vendor, product) = ids(guid);
        return HIDAPI_LAYOUTS
            .iter()
            .find(|(v, p, _)| (*v, *p) == (vendor, product))
            .map(|(_, _, mapping)| *mapping);
    }
    (mapping.contains("a:") && mapping.contains("leftx:")).then_some(mapping)
}

struct Controller<'a> {
    observed: &'a SdlController,
    guid: [u8; 16],
    port: usize,
    mapping: Option<&'a str>,
}

fn controllers(observed: &[SdlController]) -> (Vec<Controller<'_>>, bool) {
    let mut controllers: Vec<_> = observed
        .iter()
        .map(|pad| {
            let mut guid = pad.identity.guid;
            guid[2..4].fill(0);
            // SDL2 tags every HIDAPI device as USB, including Bluetooth devices.
            if guid[14] == b'h' {
                guid[0..2].copy_from_slice(&3u16.to_le_bytes());
            }
            Controller {
                observed: pad,
                guid,
                port: 0,
                mapping: sdl2_mapping(pad),
            }
        })
        .collect();
    let mut groups = BTreeMap::<_, Vec<usize>>::new();
    for (index, controller) in controllers.iter().enumerate() {
        groups.entry(controller.guid).or_default().push(index);
    }
    for indices in groups.values_mut() {
        // Assign Eden's device ports before choosing Player 1.
        let order: Option<Vec<_>> = indices
            .iter()
            .map(|i| linux_order(controllers[*i].observed))
            .collect();
        if let Some(order) = order {
            let mut sorted: Vec<_> = indices.iter().copied().zip(order).collect();
            sorted.sort_by_key(|(_, key)| *key);
            *indices = sorted.into_iter().map(|(i, _)| i).collect();
        }
        for (port, index) in indices.iter().enumerate() {
            controllers[*index].port = port;
        }
    }
    let steam_virtual = controllers.iter().any(|c| ids(c.guid) == (0x28de, 0x11ff));
    controllers.retain(|c| !steam_virtual || ids(c.guid) != (0x28de, 0x1205));
    if let Some((index, _)) = controllers
        .iter()
        .enumerate()
        .filter(|(_, c)| c.observed.last_active.is_some())
        .max_by_key(|(_, c)| c.observed.last_active)
    {
        let active = controllers.remove(index);
        controllers.insert(0, active);
    }
    (controllers, steam_virtual)
}

fn linux_order(controller: &SdlController) -> Option<(u8, u32)> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    if ids(controller.identity.guid) == (0x28de, 0x11ff) {
        return controller.player_index.map(|slot| (0, u32::from(slot)));
    }
    let event = controller
        .identity
        .path
        .as_deref()?
        .strip_prefix("/dev/input/event")?;
    let number: u32 = event.parse().ok()?;
    // SDL2 sorts evdev devices by their js index, falling back to event number.
    let js = std::fs::read_dir(format!("/sys/class/input/event{number}/device"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()?
                .strip_prefix("js")?
                .parse::<u32>()
                .ok()
        })
        .min();
    Some(js.map_or((2, number), |number| (1, number)))
}

// SDL uses raw joystick indices here, not standardized Gamepad button/axis numbers.
const BUTTONS: [(&str, &str); 22] = [
    ("a", "b"),
    ("b", "a"),
    ("x", "y"),
    ("y", "x"),
    ("lstick", "leftstick"),
    ("rstick", "rightstick"),
    ("l", "leftshoulder"),
    ("r", "rightshoulder"),
    ("zl", "lefttrigger"),
    ("zr", "righttrigger"),
    ("plus", "start"),
    ("minus", "back"),
    ("dleft", "dpleft"),
    ("dup", "dpup"),
    ("dright", "dpright"),
    ("ddown", "dpdown"),
    ("slleft", "leftshoulder"),
    ("srleft", "rightshoulder"),
    ("home", "guide"),
    ("screenshot", "misc1"),
    ("slright", "leftshoulder"),
    ("srright", "rightshoulder"),
];

fn button(binding: &str) -> Option<String> {
    if let Some(index) = binding.strip_prefix('b') {
        return Some(format!("button:{}", index.parse::<u32>().ok()?));
    }
    if let Some(hat) = binding.strip_prefix('h') {
        let (index, mask) = hat.split_once('.')?;
        let direction = match mask {
            "1" => "up",
            "2" => "right",
            "4" => "down",
            "8" => "left",
            _ => return None,
        };
        return Some(format!(
            "hat:{},direction:{direction}",
            index.parse::<u32>().ok()?
        ));
    }
    let (binding, negative) = if let Some(axis) = binding.strip_prefix('-') {
        (axis, true)
    } else {
        (binding.strip_prefix('+').unwrap_or(binding), false)
    };
    let (index, inverted) = axis(binding)?;
    let invert = if negative ^ inverted { "-" } else { "+" };
    Some(format!("axis:{index},threshold:0.5,invert:{invert}"))
}

fn axis(binding: &str) -> Option<(u32, bool)> {
    let index = binding.strip_prefix('a')?;
    let inverted = index.ends_with('~');
    Some((index.trim_end_matches('~').parse().ok()?, inverted))
}

fn guid_hex(guid: [u8; 16]) -> String {
    guid.iter().map(|b| format!("{b:02x}")).collect()
}

fn player_settings(mapping: &str, device: &str) -> Settings {
    let bindings: BTreeMap<_, _> = mapping
        .split(',')
        .skip(2)
        .filter_map(|field| field.split_once(':'))
        .collect();
    let mut settings = Settings::from([
        ("connected".into(), "true".into()),
        ("type".into(), "0".into()),
    ]);
    for (name, sdl) in BUTTONS {
        let value = bindings.get(sdl).and_then(|binding| button(binding));
        settings.insert(
            format!("button_{name}"),
            value.map_or_else(|| "engine:null".into(), |value| format!("{device},{value}")),
        );
    }
    for (name, x, y) in [("lstick", "leftx", "lefty"), ("rstick", "rightx", "righty")] {
        let axes = bindings
            .get(x)
            .and_then(|x| axis(x))
            .zip(bindings.get(y).and_then(|y| axis(y)));
        let value = axes.map_or_else(
            || "engine:null".into(),
            |((x, invert_x), (y, invert_y))| {
                let invert_x = if invert_x { "-" } else { "+" };
                let invert_y = if invert_y { "-" } else { "+" };
                format!("{device},axis_x:{x},axis_y:{y},invert_x:{invert_x},invert_y:{invert_y},deadzone:0.15,range:1.0")
            },
        );
        settings.insert(name.into(), value);
    }
    settings
}

fn config_text(existing: &str, observed: &[SdlController], menu: bool) -> (String, Prepared) {
    let (controllers, steam_virtual) = controllers(observed);
    let missing = controllers.is_empty() || controllers.iter().any(|c| c.mapping.is_none());
    let mut usable = controllers.iter().filter_map(|c| Some((c, c.mapping?)));
    let mut settings = Settings::new();
    for player in 0..=MAX_PLAYERS {
        let prefix = format!("player_{player}_");
        let controller = if player < MAX_PLAYERS {
            usable.next()
        } else {
            None
        };
        let values = controller
            .map(|(controller, mapping)| {
                let device = format!(
                    "engine:sdl,guid:{},port:{}",
                    guid_hex(controller.guid),
                    controller.port
                );
                player_settings(mapping, &device)
            })
            .unwrap_or_else(|| Settings::from([("connected".into(), "false".into())]));
        settings.insert(format!("{prefix}profile_name"), String::new());
        for (key, value) in values {
            let key = format!("{prefix}{key}");
            settings.insert(format!("{key}\\default"), "false".into());
            settings.insert(key, value);
        }
    }
    for driver in ["enable_joycon_driver", "enable_procon_driver"] {
        settings.insert(driver.into(), "false".into());
        settings.insert(format!("{driver}\\default"), "false".into());
    }
    let text = replace_settings(existing, "Controls", &settings);
    let text = replace_settings(
        &text,
        "System",
        &Settings::from([
            ("use_docked_mode".into(), "1".into()),
            ("use_docked_mode\\default".into(), "false".into()),
        ]),
    );
    // Select+Start asks to close Eden, returning to Romlin, unless it opens Romlin's menu.
    // Stop and Restart contain that chord.
    let mut ui = Settings::from([
        ("confirmStop".into(), "0".into()),
        ("confirmStop\\default".into(), "false".into()),
    ]);
    for (action, chord) in [
        ("Exit%20Eden", if menu { "" } else { "Plus+Minus" }),
        ("Stop%20Emulation", ""),
        ("Restart%20Emulation", ""),
    ] {
        let key = format!("Shortcuts\\Main%20Window\\{action}\\Controller_KeySeq");
        ui.insert(format!("{key}\\default"), "false".into());
        ui.insert(key, chord.into());
    }
    let text = replace_settings(&text, "UI", &ui);
    (
        text,
        Prepared {
            missing,
            steam_virtual,
        },
    )
}

fn replace_settings(existing: &str, section: &str, settings: &BTreeMap<String, String>) -> String {
    let header = format!("[{section}]");
    let mut result = String::new();
    let mut preserved = String::new();
    let mut in_section = false;
    for line in existing.lines() {
        if line.trim_start().starts_with('[') {
            in_section = line.trim() == header;
            if in_section {
                continue;
            }
        }
        if in_section {
            let key = line.split_once('=').map_or("", |(key, _)| key.trim());
            if !settings.contains_key(key) {
                preserved.push_str(line);
                preserved.push('\n');
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    let mut result = format!("{}\n\n{header}\n", result.trim_end());
    if !preserved.trim_end().is_empty() {
        result.push_str(preserved.trim_end());
        result.push('\n');
    }
    for (key, value) in settings {
        result.push_str(&format!("{key}={value}\n"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepads::SdlIdentity;
    use std::time::Instant;

    const STEAM: &str = "03000000de280000ff11000001000000,Steam Virtual Gamepad,\
        a:b0,b:b1,x:b2,y:b3,back:b6,start:b7,guide:b8,leftstick:b9,rightstick:b10,\
        leftshoulder:b4,rightshoulder:b5,dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,\
        leftx:a0,lefty:a1,rightx:a3,righty:a4,lefttrigger:a2,righttrigger:a5,";
    const DUALSENSE_SDL3: &str =
        "030000004c050000e60c000000016800,DualSense Wireless Controller,a:b0,b:b1,x:b2,y:b3,\
        back:b4,guide:b5,start:b6,leftstick:b7,rightstick:b8,leftshoulder:b9,rightshoulder:b10,\
        dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,touchpad:b11,misc1:b12,\
        leftx:a0,lefty:a1,rightx:a2,righty:a3,lefttrigger:a4,righttrigger:a5,";

    const DUALSHOCK4_SDL3: &str =
        "030000004c050000c405000000016800,PS4 Controller,a:b0,b:b1,x:b2,y:b3,\
        back:b4,guide:b5,start:b6,leftstick:b7,rightstick:b8,leftshoulder:b9,rightshoulder:b10,\
        dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,touchpad:b11,\
        leftx:a0,lefty:a1,rightx:a2,righty:a3,lefttrigger:a4,righttrigger:a5,";

    fn pad(hex: &str, mapping: &str) -> SdlController {
        let guid = std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap());
        SdlController {
            identity: SdlIdentity { guid, path: None },
            mapping: Some(mapping.into()),
            last_active: None,
            player_index: None,
        }
    }

    fn setting<'a>(text: &'a str, section: &str, key: &str) -> &'a str {
        let mut current = "";
        text.lines()
            .find_map(|line| {
                if let Some(header) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    current = header;
                }
                let (k, value) = line.split_once('=')?;
                (current == section && k == key).then_some(value)
            })
            .unwrap_or_else(|| panic!("Missing [{section}] {key}"))
    }

    fn control<'a>(text: &'a str, key: &str) -> &'a str {
        assert_eq!(
            setting(text, "Controls", &format!("{key}\\default")),
            "false"
        );
        setting(text, "Controls", key)
    }

    #[test]
    fn steam_bindings_are_exact_and_repeated_writes_preserve_other_settings() {
        let pads = [pad("0300abcdde280000ff11000001000000", STEAM)];
        let original = "[Renderer]\nbackend=1\n[Controls]\nplayer_0_profile_name=custom\nenable_procon_driver=true\nenable_procon_driver\\default=true\nenable_joycon_driver=true\n\
            [UI]\nfullscreen=true\n\
            Shortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq=Home+Minus\n\
            Shortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq\\default=true\n\
            Shortcuts\\Main%20Window\\Stop%20Emulation\\Controller_KeySeq=L+Plus+Minus\n\
            Shortcuts\\Main%20Window\\Restart%20Emulation\\Controller_KeySeq=R+Plus+Minus\n\
            confirmStop=2\nconfirmStop\\default=true\n";
        let (text, result) = config_text(original, &pads, false);
        assert!(!result.missing);
        assert!(result.steam_virtual);
        assert_eq!(setting(&text, "Renderer", "backend"), "1");
        assert_eq!(setting(&text, "Controls", "player_0_profile_name"), "");
        assert_eq!(control(&text, "player_0_connected"), "true");
        assert_eq!(control(&text, "player_0_type"), "0");
        let device = "engine:sdl,guid:03000000de280000ff11000001000000,port:0";
        for (key, binding) in [
            ("button_a", "button:1"),
            ("button_b", "button:0"),
            ("button_zl", "axis:2,threshold:0.5,invert:+"),
            ("button_zr", "axis:5,threshold:0.5,invert:+"),
            ("button_dup", "hat:0,direction:up"),
            (
                "lstick",
                "axis_x:0,axis_y:1,invert_x:+,invert_y:+,deadzone:0.15,range:1.0",
            ),
            (
                "rstick",
                "axis_x:3,axis_y:4,invert_x:+,invert_y:+,deadzone:0.15,range:1.0",
            ),
        ] {
            assert_eq!(
                control(&text, &format!("player_0_{key}")),
                format!("{device},{binding}")
            );
        }
        for driver in ["enable_joycon_driver", "enable_procon_driver"] {
            assert_eq!(control(&text, driver), "false");
        }
        assert_eq!(setting(&text, "System", "use_docked_mode"), "1");
        assert_eq!(
            setting(&text, "System", "use_docked_mode\\default"),
            "false"
        );
        assert_eq!(setting(&text, "UI", "fullscreen"), "true");
        for (key, value) in [
            (
                "Shortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq",
                "Plus+Minus",
            ),
            (
                "Shortcuts\\Main%20Window\\Stop%20Emulation\\Controller_KeySeq",
                "",
            ),
            (
                "Shortcuts\\Main%20Window\\Restart%20Emulation\\Controller_KeySeq",
                "",
            ),
            ("confirmStop", "0"),
        ] {
            assert_eq!(setting(&text, "UI", key), value);
            assert_eq!(setting(&text, "UI", &format!("{key}\\default")), "false");
        }
        assert_eq!(config_text(&text, &pads, false).0, text);
        for section in ["Controls", "System", "Renderer", "UI"] {
            assert_eq!(text.matches(&format!("[{section}]")).count(), 1);
        }
        // With Romlin's menu, Select+Start opens it rather than closing Eden.
        let exit = "Shortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq";
        let (with_menu, _) = config_text(&text, &pads, true);
        assert_eq!(setting(&with_menu, "UI", exit), "");
        assert_eq!(
            setting(&with_menu, "UI", &format!("{exit}\\default")),
            "false"
        );
        assert_eq!(
            setting(&config_text(&with_menu, &pads, false).0, "UI", exit),
            "Plus+Minus"
        );
    }

    #[test]
    fn sony_and_deck_hidapi_use_sdl2_buttons_instead_of_sdl3_hats() {
        // Real Sony HIDAPI GUIDs: DS4, DS4 v2, DualSense and DualSense Edge.
        for hex in [
            "030000004c050000c405000000016800",
            "030000004c050000cc09000000016800",
            "030000004c050000e60c000000016800",
            "030000004c050000f20d000000016800",
            "03000000de2800000512000011016800",
        ] {
            let mapping = if hex[16..20] == *"c405" || hex[16..20] == *"cc09" {
                DUALSHOCK4_SDL3
            } else {
                DUALSENSE_SDL3
            };
            let pads = [pad(hex, mapping)];
            let (text, result) = config_text("", &pads, false);
            assert!(!result.missing, "{hex}");
            let device = format!("engine:sdl,guid:{hex},port:0");
            for (key, button) in [
                ("a", 1),
                ("b", 0),
                ("dup", 11),
                ("ddown", 12),
                ("dleft", 13),
                ("dright", 14),
                ("screenshot", 15),
            ] {
                assert_eq!(
                    control(&text, &format!("player_0_button_{key}")),
                    format!("{device},button:{button}")
                );
            }
            assert_eq!(
                control(&text, "player_0_rstick"),
                format!("{device},axis_x:2,axis_y:3,invert_x:+,invert_y:+,deadzone:0.15,range:1.0")
            );
            assert_eq!(
                control(&text, "player_0_button_zl"),
                format!("{device},axis:4,threshold:0.5,invert:+")
            );
            assert_eq!(
                control(&text, "player_0_button_zr"),
                format!("{device},axis:5,threshold:0.5,invert:+")
            );
        }
    }

    #[test]
    fn bluetooth_hidapi_guids_use_sdl2s_usb_bus() {
        let pads = [pad("050000004c050000e60c000000016800", DUALSENSE_SDL3)];
        let (text, result) = config_text("", &pads, false);
        assert!(!result.missing);
        assert_eq!(
            control(&text, "player_0_button_a"),
            "engine:sdl,guid:030000004c050000e60c000000016800,port:0,button:1"
        );
    }

    #[test]
    fn nintendo_uses_sdl_with_vertical_uncombined_joycons() {
        for (hex, key, binding) in [
            (
                "030000007e0500000920000000006803",
                "button_dup",
                "button:11",
            ),
            (
                "030000007e0500000620000000006801",
                "button_dup",
                "button:11",
            ),
            ("030000007e0500000720000000006802", "button_a", "button:1"),
        ] {
            let (text, result) = config_text(
                "[Controls]\nenable_procon_driver=true\n",
                &[pad(hex, DUALSENSE_SDL3)],
                false,
            );
            assert!(!result.missing);
            assert_eq!(control(&text, "enable_procon_driver"), "false");
            assert_eq!(control(&text, "enable_joycon_driver"), "false");
            assert_eq!(
                control(&text, &format!("player_0_{key}")),
                format!("engine:sdl,guid:{hex},port:0,{binding}")
            );
        }
    }

    #[test]
    fn unknown_hidapi_and_missing_mappings_are_unusable() {
        let mut pads = [pad("03000000341200007856000000016800", DUALSENSE_SDL3)];
        let (text, result) = config_text("", &pads, false);
        assert!(result.missing);
        assert_eq!(control(&text, "player_0_connected"), "false");
        pads[0].mapping = None;
        assert!(config_text("", &pads, false).1.missing);
        assert!(config_text("", &[], false).1.missing);
    }

    #[test]
    fn unusable_first_pad_does_not_consume_player_one_or_renumber_device_ports() {
        let supported = pad("030000004c050000c405000000016800", DUALSHOCK4_SDL3);
        let unsupported = pad("03000000341200007856000000016800", DUALSHOCK4_SDL3);
        let mut unmapped = supported.clone();
        unmapped.mapping = None;
        for (first, port) in [(unsupported, 0), (unmapped, 1)] {
            let (text, result) = config_text("", &[first, supported.clone()], false);
            assert!(result.missing);
            assert_eq!(control(&text, "player_0_connected"), "true");
            assert_eq!(
                control(&text, "player_0_button_a"),
                format!("engine:sdl,guid:030000004c050000c405000000016800,port:{port},button:1")
            );
            assert_eq!(control(&text, "player_1_connected"), "false");
            assert_eq!(control(&text, "player_8_connected"), "false");
        }
    }

    #[test]
    fn ports_are_assigned_before_active_player_and_deck_deduplication() {
        let mut pads = vec![
            pad("03000000de280000ff11000001000000", STEAM),
            pad("03000000de2800000512000011016800", DUALSENSE_SDL3),
            pad("0300abcdde280000ff11000001000000", STEAM),
        ];
        pads[0].player_index = Some(0);
        pads[2].player_index = Some(1);
        pads[2].last_active = Some(Instant::now());
        let (text, result) = config_text("", &pads, false);
        assert!(!result.missing); // The shadowed Deck isn't an unusable extra player.
        assert!(result.steam_virtual);
        assert_eq!(
            control(&text, "player_0_button_a"),
            "engine:sdl,guid:03000000de280000ff11000001000000,port:1,button:1"
        );
        assert_eq!(
            control(&text, "player_1_button_a"),
            "engine:sdl,guid:03000000de280000ff11000001000000,port:0,button:1"
        );
        assert_eq!(control(&text, "player_2_connected"), "false");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn evdev_ports_follow_numeric_device_paths_after_hotplug() {
        let mut pads = vec![pad("030000005e0400008e02000014010000", STEAM); 2];
        pads[0].identity.path = Some("/dev/input/event9001".into());
        pads[1].identity.path = Some("/dev/input/event9000".into());
        let (controllers, _) = controllers(&pads);
        assert_eq!(controllers[0].port, 1);
        assert_eq!(controllers[1].port, 0);
    }

    #[test]
    fn eight_players_connect_and_handheld_stays_disconnected() {
        let pads = vec![pad("03000000de280000ff11000001000000", STEAM); 9];
        let (text, result) = config_text("", &pads, false);
        assert!(!result.missing);
        for player in 0..8 {
            assert_eq!(
                control(&text, &format!("player_{player}_connected")),
                "true"
            );
            assert_eq!(control(&text, &format!("player_{player}_type")), "0");
            assert_eq!(
                control(&text, &format!("player_{player}_button_a")),
                format!("engine:sdl,guid:03000000de280000ff11000001000000,port:{player},button:1")
            );
        }
        assert_eq!(control(&text, "player_8_connected"), "false");
    }
}
