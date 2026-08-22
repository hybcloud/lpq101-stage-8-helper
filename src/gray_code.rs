pub const STATE_COUNT: usize = 126;

const BOX_COUNT: u32 = 9;
const OCCUPIED_COUNT: u32 = 5;
const INITIAL_MASK: u16 = (1_u16 << OCCUPIED_COUNT) - 1;

// The filtered binary-reflected Gray code is invariant under bit permutation.
// This permutation keeps the public box numbers unchanged while shortening the
// route on the Stage 8 layout. The sequence is rotated to start at boxes 1-5.
const LOGICAL_TO_BOX: [u8; BOX_COUNT as usize] = [4, 2, 7, 5, 3, 6, 8, 9, 1];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrayState {
    occupied_mask: u16,
}

impl GrayState {
    pub const fn occupied_mask(self) -> u16 {
        self.occupied_mask
    }

    pub fn occupied_boxes(self) -> [u8; OCCUPIED_COUNT as usize] {
        let mut boxes = [0; OCCUPIED_COUNT as usize];
        let mut output = 0;
        for box_index in 0..BOX_COUNT {
            if self.occupied_mask & (1 << box_index) != 0 {
                boxes[output] = box_index as u8 + 1;
                output += 1;
            }
        }
        debug_assert_eq!(output, OCCUPIED_COUNT as usize);
        boxes
    }

    pub fn movement_to(self, next: Self) -> Movement {
        let changed = self.occupied_mask ^ next.occupied_mask;
        assert_eq!(
            changed.count_ones(),
            2,
            "adjacent Gray-code states must exchange exactly one box"
        );

        let source = changed & self.occupied_mask;
        let target = changed & next.occupied_mask;
        assert_eq!(source.count_ones(), 1);
        assert_eq!(target.count_ones(), 1);

        Movement {
            from_box: source.trailing_zeros() as u8 + 1,
            to_box: target.trailing_zeros() as u8 + 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Movement {
    pub from_box: u8,
    pub to_box: u8,
}

pub fn generate_states() -> Vec<GrayState> {
    let mut states = (0_u16..1_u16 << BOX_COUNT)
        .map(|binary| binary ^ (binary >> 1))
        .filter(|gray| gray.count_ones() == OCCUPIED_COUNT)
        .map(|gray| GrayState {
            occupied_mask: remap_to_boxes(gray),
        })
        .collect::<Vec<_>>();

    let start = states
        .iter()
        .position(|state| state.occupied_mask == INITIAL_MASK)
        .expect("the Gray code contains the initial 1-5 combination");
    states.rotate_left(start);

    // Keep 1-5 as the first state and take the shorter direction around the
    // cyclic code. This makes the first instruction 4 -> 7.
    states[1..].reverse();

    debug_assert_eq!(states.len(), STATE_COUNT);
    debug_assert!(
        states
            .windows(2)
            .all(|pair| (pair[0].occupied_mask ^ pair[1].occupied_mask).count_ones() == 2)
    );
    states
}

fn remap_to_boxes(logical_mask: u16) -> u16 {
    LOGICAL_TO_BOX
        .into_iter()
        .enumerate()
        .filter(|(logical_index, _)| logical_mask & (1 << logical_index) != 0)
        .fold(0, |mask, (_, box_number)| mask | (1 << (box_number - 1)))
}

fn format_boxes(boxes: [u8; OCCUPIED_COUNT as usize]) -> String {
    boxes
        .into_iter()
        .map(|box_number| box_number.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn format_init(state: GrayState) -> String {
    format!(
        "State 1/{STATE_COUNT} · Occupied {{{}}} · Initial setup",
        format_boxes(state.occupied_boxes())
    )
}

pub fn format_move(state_number: usize, before: GrayState, after: GrayState) -> String {
    assert!((1..=STATE_COUNT).contains(&state_number));
    let movement = before.movement_to(after);
    format!(
        "State {state_number}/{STATE_COUNT} · Occupied {{{}}} · Move box {} to box {}",
        format_boxes(after.occupied_boxes()),
        movement.from_box,
        movement.to_box
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn sequence_covers_every_five_of_nine_combination_once() {
        let states = generate_states();
        let masks = states
            .iter()
            .map(|state| state.occupied_mask())
            .collect::<HashSet<_>>();
        let expected = (0_u16..1_u16 << BOX_COUNT)
            .filter(|mask| mask.count_ones() == OCCUPIED_COUNT)
            .collect::<HashSet<_>>();

        assert_eq!(states.len(), STATE_COUNT);
        assert_eq!(masks, expected);
    }

    #[test]
    fn every_transition_moves_exactly_one_player() {
        let states = generate_states();
        for pair in states.windows(2) {
            let movement = pair[0].movement_to(pair[1]);
            assert_ne!(movement.from_box, movement.to_box);
        }
        assert_eq!(
            (states[STATE_COUNT - 1].occupied_mask() ^ states[0].occupied_mask()).count_ones(),
            2,
            "the generated Gray code is cyclic"
        );
    }

    #[test]
    fn sequence_starts_with_an_intuitive_single_move() {
        let states = generate_states();
        assert_eq!(states[0].occupied_boxes(), [1, 2, 3, 4, 5]);
        assert_eq!(
            states[0].movement_to(states[1]),
            Movement {
                from_box: 4,
                to_box: 7,
            }
        );
        assert_eq!(
            format_init(states[0]),
            "State 1/126 · Occupied {1,2,3,4,5} · Initial setup"
        );
        assert_eq!(
            format_move(2, states[0], states[1]),
            "State 2/126 · Occupied {1,2,3,5,7} · Move box 4 to box 7"
        );
    }
}
