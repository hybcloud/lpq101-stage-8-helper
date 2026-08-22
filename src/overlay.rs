use anyhow::{Context as _, Result};
use image::ImageReader;
use std::io::Cursor;

const LAYOUT_PNG: &[u8] = include_bytes!("../assets/stage8_chairs_layout.png");
const CHAIR_WIDTH: u32 = 63;
const CHAIR_HEIGHT: u32 = 41;
const CHAIR_POSITIONS: [(u32, u32); 9] = [
    (0, 0),
    (67, 0),
    (0, 39),
    (67, 39),
    (134, 39),
    (1, 78),
    (67, 78),
    (134, 78),
    (205, 78),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayVisual {
    Setup,
    State {
        occupied_mask: u16,
        movement: Option<(u8, u8)>,
    },
}

pub struct OverlayImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl OverlayImage {
    pub fn load() -> Result<Self> {
        let image = ImageReader::new(Cursor::new(LAYOUT_PNG))
            .with_guessed_format()?
            .decode()
            .context("decode embedded Stage 8 layout")?
            .to_rgba8();
        let (width, height) = image.dimensions();
        anyhow::ensure!(
            (width, height) == (268, 119),
            "unexpected Stage 8 image dimensions"
        );
        Ok(Self {
            width,
            height,
            rgba: image.into_raw(),
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn compose_bgra(&self, visual: OverlayVisual) -> Vec<u8> {
        let mut output = vec![0_u8; (self.width * self.height * 4) as usize];
        // A non-zero but visually negligible alpha makes the transparent gaps
        // participate in Win32 layered-window hit testing. The whole image can
        // therefore be dragged, and every outer edge remains reachable for resize.
        for pixel in output.chunks_exact_mut(4) {
            pixel[3] = 1;
        }
        for (index, &(origin_x, origin_y)) in CHAIR_POSITIONS.iter().enumerate() {
            let tint = match visual {
                OverlayVisual::Setup => ChairTint::Setup,
                OverlayVisual::State {
                    occupied_mask,
                    movement,
                } => {
                    let box_number = index as u8 + 1;
                    match movement {
                        Some((source, _)) if box_number == source => ChairTint::Source,
                        Some((_, target)) if box_number == target => ChairTint::Target,
                        _ if occupied_mask & (1 << index) != 0 => ChairTint::Occupied,
                        _ => ChairTint::Empty,
                    }
                }
            };
            for y in origin_y..origin_y + CHAIR_HEIGHT {
                for x in origin_x..origin_x + CHAIR_WIDTH {
                    let offset = ((y * self.width + x) * 4) as usize;
                    let red = self.rgba[offset];
                    let green = self.rgba[offset + 1];
                    let blue = self.rgba[offset + 2];
                    let alpha = self.rgba[offset + 3];
                    if alpha == 0 {
                        continue;
                    }

                    let gray =
                        ((red as u32 * 299 + green as u32 * 587 + blue as u32 * 114) / 1000) as u8;
                    let (tinted_red, tinted_green, tinted_blue) = match tint {
                        ChairTint::Setup => (gray, gray, gray),
                        ChairTint::Empty => (scale(gray, 112), scale(gray, 112), scale(gray, 112)),
                        ChairTint::Occupied => (scale(gray, 36), scale(gray, 188), gray),
                        ChairTint::Source => (gray, scale(gray, 44), scale(gray, 44)),
                        ChairTint::Target => (scale(gray, 36), gray, scale(gray, 68)),
                    };

                    // Direct2D and UpdateLayeredWindow both expect premultiplied BGRA.
                    output[offset] = premultiply(tinted_blue, alpha);
                    output[offset + 1] = premultiply(tinted_green, alpha);
                    output[offset + 2] = premultiply(tinted_red, alpha);
                    output[offset + 3] = alpha;
                }
            }
        }
        output
    }
}

#[derive(Clone, Copy)]
enum ChairTint {
    Setup,
    Occupied,
    Empty,
    Source,
    Target,
}

fn scale(value: u8, factor: u8) -> u8 {
    (value as u16 * factor as u16 / 255) as u8
}

fn premultiply(value: u8, alpha: u8) -> u8 {
    (value as u16 * alpha as u16 / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_layout_composes_gray_and_colored_images() {
        let image = OverlayImage::load().unwrap();
        let gray = image.compose_bgra(OverlayVisual::Setup);
        let transition = image.compose_bgra(OverlayVisual::State {
            occupied_mask: 0b001_010_111,
            movement: Some((4, 7)),
        });
        assert_eq!(gray.len(), (268 * 119 * 4) as usize);
        assert_eq!(transition.len(), gray.len());
        assert!(gray.chunks_exact(4).all(|pixel| pixel[3] > 0));
        assert!(
            gray.chunks_exact(4)
                .filter(|pixel| pixel[3] > 0)
                .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2])
        );
        assert!(
            transition
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[0] > pixel[1] && pixel[1] > pixel[2])
        );
        assert!(
            transition
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[2] > pixel[1] && pixel[2] > pixel[0])
        );
        assert!(
            transition
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[1] > pixel[2] && pixel[1] > pixel[0])
        );
        assert!(
            transition
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[0] == pixel[1] && pixel[1] == pixel[2])
        );
    }
}
