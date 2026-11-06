//! Buzz! handsets as the game's controller ports see them.

use crate::input;
use romlin_proto::msg::PadState;

/// Bit `n` of a handset's mask is its button `n`, in the order the receiver reports them.
pub const RED: u32 = 0;
pub const YELLOW: u32 = 1;
pub const GREEN: u32 = 2;
pub const ORANGE: u32 = 3;
pub const BLUE: u32 = 4;
pub const BUTTONS: u32 = 5;

/// The RetroPad buttons a handset's pressed buttons press. Start and Select stay free.
pub fn pad(mask: u8) -> PadState {
    const MAP: [(u32, u32); 5] = [
        (RED, input::A),
        (YELLOW, input::Y),
        (GREEN, input::X),
        (ORANGE, input::B),
        (BLUE, input::R),
    ];
    let buttons = MAP
        .iter()
        .filter(|(bit, _)| (mask >> bit) & 1 == 1)
        .fold(0, |buttons, (_, pad)| buttons | 1 << pad);
    PadState {
        buttons,
        axes: [0; 6],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_handset_button_presses_its_retropad_button() {
        let pressed = |bit: u32| pad(1 << bit).buttons;
        assert_eq!(pressed(RED), 1 << input::A);
        assert_eq!(pressed(YELLOW), 1 << input::Y);
        assert_eq!(pressed(GREEN), 1 << input::X);
        assert_eq!(pressed(ORANGE), 1 << input::B);
        assert_eq!(pressed(BLUE), 1 << input::R);
    }

    #[test]
    fn simultaneous_presses_combine_and_idle_is_empty() {
        let all = (1 << BUTTONS) - 1;
        assert_eq!(
            pad(all).buttons,
            1 << input::A | 1 << input::Y | 1 << input::X | 1 << input::B | 1 << input::R
        );
        assert_eq!(pad(0), PadState::default());
    }
}
