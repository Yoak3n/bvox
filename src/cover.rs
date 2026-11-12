//! Fallback cover rendering: decode to a small RGB grid for half-block cells.
//! Terminals with kitty/sixel support use `ratatui-image` instead.

use anyhow::Result;
use image::DynamicImage;
use image::imageops::FilterType;

pub struct Cover {
    pub width: usize,
    pub height: usize,
    /// Row-major RGB pixels, `width * height` entries.
    pub pixels: Vec<(u8, u8, u8)>,
}

pub struct CoverSet {
    pub small: Cover,
    pub large: Cover,
}

pub fn from_dynamic(image: &DynamicImage, width: u32, height: u32) -> Result<Cover> {
    let image = image.resize_exact(width, height, FilterType::Lanczos3);
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();

    Ok(Cover {
        width: width as usize,
        height: height as usize,
        pixels: rgb
            .pixels()
            .map(|pixel| (pixel[0], pixel[1], pixel[2]))
            .collect(),
    })
}
