use crate::gamepads::SdlController;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

type Settings = BTreeMap<String, String>;
const MAX_PLAYERS: usize = 8;

/// Refresh Romlin's controller setup, preserving players customized in Eden.
pub fn prepare(exe: &Path, controllers: &[SdlController]) -> io::Result<()> {
    let dir = exe
        .parent()
        .ok_or_else(|| io::Error::other("Missing Eden directory"))?
        .join("user/config");
    let config = dir.join("qt-config.ini");
    // Keep the ownership snapshot name so existing custom controller settings stay protected.
    let state = dir.join("romp-input.json");
    let existing = match std::fs::read_to_string(&config) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let previous = match std::fs::read(&state) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Settings::new(),
        Err(e) => return Err(e),
    };
    let Some((text, managed)) = config_text(&existing, controllers, &previous) else {
        let guids: Vec<String> = controllers.iter().map(|c| guid_hex(c.guid)).collect();
        tracing::info!(?guids, "Eden controls left unchanged");
        return Ok(());
    };
    let players: Vec<String> = managed
        .iter()
        .filter(|(key, value)| key.ends_with("_connected") && *value == "true")
        .map(|(key, _)| {
            let player = key.trim_end_matches("_connected");
            let a = managed.get(&format!("{player}_button_a"));
            format!("{player}: {}", a.map_or("", String::as_str))
        })
        .collect();
    tracing::info!(?players, "Eden controls configured");
    crate::saves::replace_file(&config, text.as_bytes())?;
    crate::saves::replace_file(
        &state,
        &serde_json::to_vec(&managed).map_err(io::Error::other)?,
    )?;
    Ok(())
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

fn read_controls(existing: &str) -> BTreeMap<&str, &str> {
    let mut in_controls = false;
    existing
        .lines()
        .filter(|line| {
            if line.trim_start().starts_with('[') {
                in_controls = line.trim() == "[Controls]";
                return false;
            }
            in_controls
        })
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim().trim_matches('"')))
        .collect()
}

// Eden serializes ParamPackage fields in its own order.
fn same_value(a: &str, b: &str) -> bool {
    if a.contains("engine:") && b.contains("engine:") {
        let params = |text: &str| -> BTreeMap<String, String> {
            text.split(',')
                .filter_map(|field| field.split_once(':'))
                .map(|(key, value)| (key.into(), value.into()))
                .collect()
        };
        params(a) == params(b)
    } else {
        a == b
    }
}

fn can_configure(controls: &BTreeMap<&str, &str>, previous: &Settings, player: usize) -> bool {
    let prefix = format!("player_{player}_");
    let mut has_nondefault_input = false;
    // A non-default input without a matching Romlin snapshot belongs to the user.
    let customized = controls.iter().any(|(key, value)| {
        let Some(field) = key.strip_prefix(&prefix) else {
            return false;
        };
        if field == "profile_name" {
            return !value.is_empty();
        }
        let Some(setting) = key.strip_suffix("\\default") else {
            return false;
        };
        let nondefault = (field.starts_with("button_")
            || field.starts_with("lstick")
            || field.starts_with("rstick")
            || field.starts_with("motion")
            || field == "type\\default")
            && *value == "false";
        has_nondefault_input |= nondefault;
        nondefault
            && !previous.get(setting).is_some_and(|old| {
                controls
                    .get(setting)
                    .is_some_and(|now| same_value(now, old))
            })
    });
    !customized
        && (!has_nondefault_input
            || previous
                .iter()
                .filter(|(key, _)| key.starts_with(&prefix))
                .all(|(key, old)| {
                    let binding = key.contains("_button_")
                        || key.ends_with("_lstick")
                        || key.ends_with("_rstick");
                    (!binding || controls.get(format!("{key}\\default").as_str()) == Some(&"false"))
                        && controls
                            .get(key.as_str())
                            .is_some_and(|now| same_value(now, old))
                }))
}

fn guid_hex(guid: [u8; 16]) -> String {
    guid.iter().map(|b| format!("{b:02x}")).collect()
}

fn player_settings(mapping: &str, device: &str, controller_type: &str) -> Settings {
    let bindings: BTreeMap<_, _> = mapping
        .split(',')
        .skip(2)
        .filter_map(|field| field.split_once(':'))
        .collect();
    let mut settings = Settings::from([
        ("connected".into(), "true".into()),
        ("type".into(), controller_type.into()),
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

fn config_text(
    existing: &str,
    controllers: &[SdlController],
    previous: &Settings,
) -> Option<(String, Settings)> {
    let controls = read_controls(existing);
    let first_owned =
        can_configure(&controls, previous, 0) && can_configure(&controls, previous, 8);
    let handheld = controllers.len() == 1;
    let mut managed = Settings::new();
    let mut ports = BTreeMap::<[u8; 16], usize>::new();
    for (player, controller) in controllers.iter().take(MAX_PLAYERS).enumerate() {
        // Eden strips the name CRC and numbers identical GUIDs from zero independently.
        let mut guid = controller.guid;
        guid[2..4].fill(0);
        let port = ports.entry(guid).or_default();
        let device = format!("engine:sdl,guid:{},port:{port}", guid_hex(guid));
        *port += 1;
        let owned = if player == 0 {
            first_owned
        } else {
            can_configure(&controls, previous, player)
        };
        let Some(mapping) = controller.mapping.as_deref().filter(|_| owned) else {
            continue;
        };
        let controller_type = if player == 0 && handheld { "4" } else { "0" };
        let mut settings = player_settings(mapping, &device, controller_type);
        if player == 0 {
            // Handheld has a separate input slot; never leave it connected alongside Player 1.
            let mut handheld_settings = settings.clone();
            handheld_settings.insert("type".into(), "4".into());
            handheld_settings.insert("connected".into(), handheld.to_string());
            managed.extend(
                handheld_settings
                    .into_iter()
                    .map(|(key, value)| (format!("player_8_{key}"), value)),
            );
            settings.insert("connected".into(), (!handheld).to_string());
        }
        managed.extend(
            settings
                .into_iter()
                .map(|(key, value)| (format!("player_{player}_{key}"), value)),
        );
    }

    // Disconnect absent devices only in slots Romlin still owns, keeping their bindings for reconnect.
    for player in 0..=MAX_PLAYERS {
        let prefix = format!("player_{player}_");
        if (matches!(player, 0 | 8) && !first_owned)
            || managed.contains_key(&format!("{prefix}connected"))
            || !can_configure(&controls, previous, player)
        {
            continue;
        }
        let old: Settings = previous
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if !old.is_empty() {
            managed.extend(old);
            managed.insert(format!("{prefix}connected"), "false".into());
        }
    }
    if managed.is_empty() {
        return None;
    }

    let mut settings = managed.clone();
    // Eden ignores input values unless their default flags are explicitly disabled.
    settings.extend(
        managed
            .keys()
            .map(|key| (format!("{key}\\default"), "false".into())),
    );
    let text = replace_settings(existing, "Controls", &settings);
    // System settings are loaded after Controls and would otherwise restore docked mode.
    let text = if handheld && managed.contains_key("player_0_type") && first_owned {
        replace_settings(
            &text,
            "System",
            &BTreeMap::from([
                ("use_docked_mode".into(), "0".into()),
                ("use_docked_mode\\default".into(), "false".into()),
            ]),
        )
    } else {
        text
    };
    Some((text, managed))
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

    // Xbox-compatible evdev layout: triggers interrupt the stick axis numbering.
    const MAPPING: &str = "030000005e0400008e02000014010000,Xbox 360 Controller,\
        a:b0,b:b1,x:b2,y:b3,back:b6,start:b7,guide:b8,leftstick:b9,rightstick:b10,\
        leftshoulder:b4,rightshoulder:b5,dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,\
        leftx:a0,lefty:a1,rightx:a3,righty:a4,lefttrigger:a2,righttrigger:a5,platform:Linux,";
    const GUID: [u8; 16] = [
        3, 0, 0xab, 0xcd, 0x5e, 4, 0, 0, 0x8e, 2, 0, 0, 0x14, 1, 0, 0,
    ];

    fn pad(guid: [u8; 16]) -> SdlController {
        SdlController {
            guid,
            mapping: Some(MAPPING.into()),
        }
    }

    fn setting<'a>(text: &'a str, key: &str) -> &'a str {
        let mut controls = false;
        text.lines()
            .find_map(|line| {
                if line.starts_with('[') {
                    controls = line == "[Controls]";
                }
                let (k, value) = line.split_once('=')?;
                (controls && k == key).then_some(value)
            })
            .unwrap()
    }

    fn input<'a>(text: &'a str, key: &str) -> BTreeMap<&'a str, &'a str> {
        // Eden ignores an input value without this flag, even if the binding is correct.
        assert_eq!(setting(text, &format!("{key}\\default")), "false");
        setting(text, key)
            .split(',')
            .map(|field| field.split_once(':').unwrap())
            .collect()
    }

    #[test]
    fn fresh_install_connects_a_handheld_with_actual_sdl_bindings() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("eden");
        prepare(&exe, &[pad(GUID)]).unwrap();
        let config = dir.path().join("user/config/qt-config.ini");
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains("[System]\nuse_docked_mode=0\nuse_docked_mode\\default=false\n"));
        assert_eq!(setting(&text, "player_0_connected"), "false");
        assert_eq!(setting(&text, "player_0_connected\\default"), "false");
        assert_eq!(setting(&text, "player_8_connected"), "true");
        assert_eq!(setting(&text, "player_8_connected\\default"), "false");
        assert_eq!(setting(&text, "player_0_type"), "4");
        assert_eq!(setting(&text, "player_0_type\\default"), "false");
        assert_eq!(setting(&text, "player_8_type"), "4");
        assert_eq!(setting(&text, "player_8_type\\default"), "false");
        let a = input(&text, "player_8_button_a");
        assert_eq!(a["engine"], "sdl");
        assert_eq!(a["guid"], "030000005e0400008e02000014010000");
        assert_eq!(a["port"], "0");
        assert_eq!(a["button"], "1");
        assert_eq!(input(&text, "player_0_button_a"), a);
        assert_eq!(
            input(&text, "player_8_lstick"),
            input(&text, "player_0_lstick")
        );
        assert_eq!(
            input(&text, "player_8_rstick"),
            input(&text, "player_0_rstick")
        );
        assert_eq!(input(&text, "player_0_button_b")["button"], "0");
        let up = input(&text, "player_0_button_dup");
        assert_eq!(up["hat"], "0");
        assert_eq!(up["direction"], "up");
        assert_eq!(input(&text, "player_0_button_zl")["axis"], "2");
        assert_eq!(input(&text, "player_0_button_zr")["axis"], "5");
        let right = input(&text, "player_0_rstick");
        assert_eq!(right["axis_x"], "3");
        assert_eq!(right["axis_y"], "4");
        assert_eq!(input(&text, "player_0_button_screenshot")["engine"], "null");
        // Repeated launches must not accumulate duplicate sections or whitespace.
        prepare(&exe, &[pad(GUID)]).unwrap();
        assert_eq!(std::fs::read_to_string(config).unwrap(), text);
    }

    #[test]
    fn repairs_edens_keyboard_defaults_without_changing_other_settings() {
        let existing = "[Renderer]\nbackend=1\n[Controls]\n\
            player_0_type=0\nplayer_0_type\\default=true\n\
            player_0_button_a=engine:keyboard,code:67\nplayer_0_button_a\\default=true\n\
            player_1_connected=true\nplayer_1_button_a=engine:sdl,button:8\n\
            touchscreen_enabled=true\n[Data%20Storage]\nnand_directory=/saves/game\n\
            [System]\nuse_docked_mode=1\nuse_docked_mode\\default=true\nlanguage_index=1\n";
        let (text, _) = config_text(existing, &[pad(GUID)], &Settings::new()).unwrap();
        assert_eq!(text.matches("[Controls]").count(), 1);
        assert_eq!(text.matches("[System]").count(), 1);
        assert!(text.contains("use_docked_mode=0\nuse_docked_mode\\default=false\n"));
        assert!(!text.contains("engine:keyboard"));
        assert_eq!(input(&text, "player_0_button_a")["button"], "1");
        for preserved in [
            "backend=1",
            "player_1_connected=true",
            "player_1_button_a=engine:sdl,button:8",
            "touchscreen_enabled=true",
            "nand_directory=/saves/game",
            "language_index=1",
        ] {
            assert!(text.contains(preserved), "{preserved}");
        }
    }

    #[test]
    fn keeps_manual_bindings_and_input_profiles() {
        for custom in [
            "player_0_button_a=engine:sdl,button:2\nplayer_0_button_a\\default=false",
            "player_0_button_a=engine:keyboard,code:65\nplayer_0_button_a\\default=false",
            "player_0_lstick\\default=false",
            "player_0_motionleft\\default=false",
            "player_8_button_a=engine:sdl,button:2\nplayer_8_button_a\\default=false",
            "player_0_profile_name=My controller",
            "player_8_profile_name=Handheld controller",
        ] {
            assert!(config_text(
                &format!("[Controls]\n{custom}\n"),
                &[pad(GUID)],
                &Settings::new()
            )
            .is_none());
        }
    }

    #[test]
    fn assigns_distinct_ports_to_identical_controllers_up_to_eight_players() {
        let mut same_model = GUID;
        same_model[2..4].fill(0); // Name CRCs must not split Eden's port counter.
        let mut pads = vec![pad(GUID), pad([7; 16]), pad(same_model)];
        pads.extend((0..6).map(|_| pad(GUID)));
        let (text, _) = config_text("", &pads, &Settings::new()).unwrap();
        for player in 0..8 {
            assert_eq!(
                setting(&text, &format!("player_{player}_connected")),
                "true"
            );
            assert_eq!(setting(&text, &format!("player_{player}_type")), "0");
        }
        assert_eq!(input(&text, "player_0_button_a")["port"], "0");
        assert_eq!(input(&text, "player_1_button_a")["port"], "0");
        assert_eq!(input(&text, "player_2_button_a")["port"], "1");
        assert_eq!(input(&text, "player_7_button_a")["port"], "6");
        assert_eq!(setting(&text, "player_8_connected"), "false");
        assert!(!text.contains("player_9_"));
    }

    #[test]
    fn switches_between_solo_and_multiplayer_across_launches() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("eden");
        let config = dir.path().join("user/config/qt-config.ini");
        prepare(&exe, &[pad(GUID)]).unwrap();
        prepare(&exe, &[pad(GUID), pad(GUID)]).unwrap();
        let text = std::fs::read_to_string(&config).unwrap();
        assert_eq!(setting(&text, "player_0_connected"), "true");
        assert_eq!(setting(&text, "player_1_connected"), "true");
        assert_eq!(setting(&text, "player_8_connected"), "false");
        assert_eq!(input(&text, "player_1_button_a")["port"], "1");

        // The surviving physical pad becomes the first controller in the next Eden process.
        prepare(&exe, &[pad(GUID)]).unwrap();
        let text = std::fs::read_to_string(&config).unwrap();
        assert_eq!(setting(&text, "player_0_connected"), "false");
        assert_eq!(setting(&text, "player_1_connected"), "false");
        assert_eq!(setting(&text, "player_8_connected"), "true");
        assert_eq!(input(&text, "player_8_button_a")["port"], "0");

        prepare(&exe, &[]).unwrap();
        let text = std::fs::read_to_string(config).unwrap();
        for player in [0, 1, 8] {
            assert_eq!(
                setting(&text, &format!("player_{player}_connected")),
                "false"
            );
        }
    }

    #[test]
    fn refreshes_automatic_bindings_after_eden_reorders_parameters() {
        let (text, previous) = config_text("", &[pad(GUID), pad(GUID)], &Settings::new()).unwrap();
        let mut rewritten = Settings::new();
        for (key, value) in read_controls(&text) {
            let value = if value.contains("engine:") {
                value.split(',').rev().collect::<Vec<_>>().join(",")
            } else {
                value.into()
            };
            rewritten.insert(key.into(), value);
        }
        // Eden marks booleans equal to their default as default even when Romlin wrote false.
        rewritten.insert("player_0_connected\\default".into(), "true".into());
        let text = replace_settings(&text, "Controls", &rewritten);
        let (text, _) = config_text(&text, &[pad([9; 16]), pad(GUID)], &previous).unwrap();
        assert_eq!(
            input(&text, "player_0_button_a")["guid"],
            "09090000090909090909090909090909"
        );
        assert_eq!(
            input(&text, "player_1_button_a")["guid"],
            "030000005e0400008e02000014010000"
        );
        assert_eq!(setting(&text, "player_0_type"), "0");
    }

    #[test]
    fn manual_changes_to_one_player_do_not_block_other_players() {
        let (text, previous) = config_text("", &[pad(GUID), pad(GUID)], &Settings::new()).unwrap();
        for change in [
            ("player_0_button_a", "engine:keyboard,code:65"),
            ("player_0_button_a\\default", "true"),
            ("player_0_type", "3"),
            ("player_0_profile_name", "My profile"),
            ("player_8_button_b", "engine:keyboard,code:66"),
        ] {
            let custom = replace_settings(
                &text,
                "Controls",
                &Settings::from([(change.0.into(), change.1.into())]),
            );
            let (next, managed) =
                config_text(&custom, &[pad(GUID), pad([9; 16])], &previous).unwrap();
            for (key, value) in read_controls(&custom) {
                if key.starts_with("player_0_") || key.starts_with("player_8_") {
                    assert_eq!(setting(&next, key), value, "{key}");
                }
            }
            assert!(!managed.contains_key("player_0_type"));
            assert_eq!(
                input(&next, "player_1_button_a")["guid"],
                "09090000090909090909090909090909"
            );
        }
    }

    #[test]
    fn custom_player_two_is_preserved_while_player_one_is_configured() {
        let existing = "[Controls]\nplayer_1_type=3\nplayer_1_type\\default=false\n\
            player_1_button_a=engine:keyboard,code:65\nplayer_1_button_a\\default=false\n";
        let (text, _) = config_text(existing, &[pad(GUID), pad(GUID)], &Settings::new()).unwrap();
        assert_eq!(setting(&text, "player_0_connected"), "true");
        assert_eq!(setting(&text, "player_1_type"), "3");
        assert_eq!(input(&text, "player_1_button_a")["engine"], "keyboard");
    }

    #[test]
    fn missing_mapping_still_reserves_its_sdl_port() {
        let pads = [
            SdlController {
                guid: GUID,
                mapping: None,
            },
            pad(GUID),
        ];
        let (text, _) = config_text("", &pads, &Settings::new()).unwrap();
        assert_eq!(input(&text, "player_1_button_a")["port"], "1");
        assert!(!text.contains("player_0_"));
        assert!(config_text("", &[], &Settings::new()).is_none());
    }

    #[test]
    fn deleted_config_can_be_rebuilt_despite_the_previous_snapshot() {
        let (_, previous) = config_text("", &[pad(GUID)], &Settings::new()).unwrap();
        let (text, _) = config_text("", &[pad([9; 16])], &previous).unwrap();
        assert_eq!(
            input(&text, "player_8_button_a")["guid"],
            "09090000090909090909090909090909"
        );
    }

    #[test]
    fn corrupt_ownership_state_does_not_overwrite_controls() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("user/config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("qt-config.ini"), "[Controls]\n").unwrap();
        std::fs::write(config.join("romp-input.json"), "invalid json").unwrap();
        assert!(prepare(&dir.path().join("eden"), &[pad(GUID)]).is_err());
        assert_eq!(
            std::fs::read_to_string(config.join("qt-config.ini")).unwrap(),
            "[Controls]\n"
        );
    }

    #[test]
    fn translates_button_hat_and_signed_axis_bindings() {
        assert_eq!(button("b12").as_deref(), Some("button:12"));
        assert_eq!(button("h2.8").as_deref(), Some("hat:2,direction:left"));
        assert_eq!(
            button("-a3").as_deref(),
            Some("axis:3,threshold:0.5,invert:-")
        );
        assert_eq!(
            button("+a2").as_deref(),
            Some("axis:2,threshold:0.5,invert:+")
        );
        assert_eq!(
            button("a5~").as_deref(),
            Some("axis:5,threshold:0.5,invert:-")
        );
        assert_eq!(axis("a1~"), Some((1, true)));
        for invalid in ["", "b-1", "h0.3", "b1\n[Controls]", "a1,engine:keyboard"] {
            assert!(button(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn config_read_errors_are_reported_without_replacing_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("user/config/qt-config.ini");
        std::fs::create_dir_all(&config).unwrap();
        assert!(prepare(&dir.path().join("eden"), &[pad(GUID)]).is_err());
        assert!(config.is_dir());
    }
}
