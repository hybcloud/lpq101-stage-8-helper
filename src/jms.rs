pub const LOGICAL_TO_BOX: [u8; 9] = [1, 3, 6, 7, 4, 8, 2, 5, 9];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JmsState {
    pub player_boxes: [u8; 5],
}

impl JmsState {
    pub fn occupied_boxes(self) -> [u8; 5] {
        let mut boxes = self.player_boxes;
        boxes.sort_unstable();
        boxes
    }

    pub fn occupied_mask(self) -> u16 {
        self.player_boxes
            .into_iter()
            .fold(0_u16, |mask, box_number| mask | (1 << (box_number - 1)))
    }
}

pub fn generate_states() -> Vec<JmsState> {
    let mut states = Vec::with_capacity(126);
    for player_b in 1..9 {
        for player_c in player_b + 1..9 {
            for player_d in player_c + 1..9 {
                for player_e in player_d + 1..9 {
                    for player_a in 0..player_b {
                        let positions = [player_a, player_b, player_c, player_d, player_e];
                        states.push(JmsState {
                            player_boxes: positions.map(|position| LOGICAL_TO_BOX[position]),
                        });
                    }
                }
            }
        }
    }
    states
}

fn format_boxes(boxes: [u8; 5], opening: char, closing: char) -> String {
    let body = boxes
        .into_iter()
        .map(|box_number| box_number.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("{opening}{body}{closing}")
}

pub fn format_init(state: JmsState) -> String {
    format!("init: {}", format_boxes(state.occupied_boxes(), '(', ')'))
}

pub fn format_step(step: usize, before: JmsState, after: JmsState) -> String {
    assert!(step >= 1, "step numbering starts at one");
    let movements = before
        .player_boxes
        .into_iter()
        .zip(after.player_boxes)
        .filter(|(source, target)| source != target)
        .map(|(source, target)| format!("{source} goto {target}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "step {step}:{}->{} ({movements})",
        format_boxes(before.occupied_boxes(), '{', '}'),
        format_boxes(after.occupied_boxes(), '{', '}')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_matches_reference() {
        let states = generate_states();
        assert_eq!(states.len(), 126);
        assert_eq!(format_init(states[0]), "init: (1,3,4,6,7)");
        assert_eq!(
            format_step(1, states[0], states[1]),
            "step 1:{1,3,4,6,7}->{1,3,6,7,8} (4 goto 8)"
        );
    }
}
