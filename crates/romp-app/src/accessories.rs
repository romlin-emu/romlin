//! USB accessories that PS2 games expect, found from the game's metadata and the devices plugged in.

use crate::store::GameDetail;
use std::path::Path;
use tracing::warn;

pub mod buzz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accessory {
    Buzz,
    SingStar,
}

impl Accessory {
    /// The value of the core's `pcsx2_usb_accessory` option.
    pub fn core_option(self) -> &'static str {
        match self {
            Self::Buzz => "buzz",
            Self::SingStar => "singstar",
        }
    }
}

/// The accessory a PS2 game needs, from RomM's franchises, the title, or the downloaded file's name.
pub fn ps2_accessory(detail: &GameDetail, rom: &Path) -> Option<Accessory> {
    let file_name = rom.file_name().and_then(|n| n.to_str());
    detail
        .meta
        .franchises
        .iter()
        .map(String::as_str)
        .chain([detail.title.as_str()])
        .chain(file_name)
        .find_map(|name| {
            let name = name.to_ascii_lowercase();
            if name.starts_with("buzz!") {
                Some(Accessory::Buzz)
            } else if name.starts_with("singstar") {
                Some(Accessory::SingStar)
            } else {
                None
            }
        })
}

/// Whether a USB device is a Buzz! receiver, whose handsets are read as joysticks.
pub fn is_buzz_receiver(vendor: u16, product: u16) -> bool {
    matches!((vendor, product), (0x054c, 0x0002) | (0x054c, 0x1000))
}

fn is_singstar_adapter(vendor: u16, product: u16) -> bool {
    matches!((vendor, product), (0x1415, 0x0000) | (0x1415, 0x0020))
}

/// The product string of the first SingStar adapter among `devices`; empty when it has none.
fn singstar_product(
    devices: impl IntoIterator<Item = (u16, u16, Option<String>)>,
) -> Option<String> {
    devices
        .into_iter()
        .find(|(vendor, product, _)| is_singstar_adapter(*vendor, *product))
        .map(|(_, _, name)| name.unwrap_or_default())
}

/// The product string of the SingStar adapter plugged in, if any. Devices are listed, never opened.
pub fn singstar_adapter() -> Option<String> {
    use nusb::MaybeFuture;
    let devices = match nusb::list_devices().wait() {
        Ok(devices) => devices,
        Err(e) => {
            warn!("couldn't list USB devices: {e}");
            return None;
        }
    };
    singstar_product(devices.map(|d| {
        (
            d.vendor_id(),
            d.product_id(),
            d.product_string().map(str::to_owned),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(title: &str, franchises: &[&str]) -> GameDetail {
        GameDetail {
            id: 1,
            title: title.into(),
            platform_id: 1,
            platform_slug: "ps2".into(),
            platform: "PlayStation 2".into(),
            platform_category: None,
            summary: None,
            size_bytes: 1,
            cover_small: None,
            cover_large: None,
            local_path: None,
            meta: crate::romm::types::RomMetadata {
                franchises: franchises.iter().map(ToString::to_string).collect(),
                ..Default::default()
            },
            screenshots: Vec::new(),
        }
    }

    #[test]
    fn a_franchise_title_or_file_name_finds_the_accessory() {
        let found = |title, franchises: &[&str], path| {
            ps2_accessory(&detail(title, franchises), Path::new(path))
        };
        assert_eq!(
            found("Anything", &["Buzz!"], "/roms/1/a.iso"),
            Some(Accessory::Buzz)
        );
        assert_eq!(
            found("Singstar: Legends", &[], "/roms/1/a.iso"),
            Some(Accessory::SingStar)
        );
        assert_eq!(
            found("Unmatched", &[], "/roms/1/BUZZ! The Mega Quiz.chd"),
            Some(Accessory::Buzz)
        );
    }

    #[test]
    fn other_games_get_no_accessory() {
        let d = detail("Gran Turismo 4", &["Gran Turismo"]);
        assert_eq!(
            ps2_accessory(&d, Path::new("/roms/2/Gran Turismo 4.iso")),
            None
        );
    }

    #[test]
    fn buzz_receivers_and_singstar_adapters_are_recognised_by_id() {
        assert!(is_buzz_receiver(0x054c, 0x0002));
        assert!(is_buzz_receiver(0x054c, 0x1000));
        assert!(
            !is_buzz_receiver(0x054c, 0x05c4),
            "a DualShock 4 is a gamepad"
        );
        assert!(is_singstar_adapter(0x1415, 0x0000));
        assert!(is_singstar_adapter(0x1415, 0x0020));
        assert!(!is_singstar_adapter(0x1415, 0x0001));
    }

    #[test]
    fn the_first_singstar_adapter_gives_the_microphone_name() {
        let devices = vec![
            (0x054c, 0x0002, Some("Buzz! Receiver".to_string())),
            (0x1415, 0x0020, Some("USBMIC Serial".to_string())),
            (0x1415, 0x0000, Some("Other Adapter".to_string())),
        ];
        assert_eq!(
            singstar_product(devices.clone()),
            Some("USBMIC Serial".into())
        );
        assert_eq!(singstar_product(devices[..1].to_vec()), None);
    }
}
