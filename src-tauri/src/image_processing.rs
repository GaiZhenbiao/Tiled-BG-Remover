use image::{GenericImageView, DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Cursor;
use base64::{Engine as _, engine::general_purpose};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TileInfo {
    pub r: u32,
    pub c: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub path: String,
}

// ... rest of split_image ...

// Helper: Check if pixel is white or transparent
fn is_white(p: &Rgba<u8>) -> bool {
    // Check alpha (transparency)
    if p[3] < 10 {
        return true;
    }
    // Check white
    p[0] >= 250 && p[1] >= 250 && p[2] >= 250
}
pub fn crop_image(input_path: &str, x: u32, y: u32, width: u32, height: u32, output_dir: &Path) -> Result<String, String> {
    let mut img = image::open(input_path).map_err(|e| e.to_string())?;
    let cropped = img.crop_imm(x, y, width, height);
    
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let file_name = format!("cropped_{}.png", timestamp);
    let file_path = output_dir.join(&file_name);
    
    cropped.to_image().save_with_format(&file_path, ImageFormat::Png).map_err(|e| e.to_string())?;
    
    Ok(file_path.to_string_lossy().to_string())
}

pub fn split_image(
    input_path: &str, 
    rows: u32, 
    cols: u32, 
    overlap_ratio: f64, 
    output_dir: &Path
) -> Result<(Vec<TileInfo>, u32, u32), String> {
    let img = image::open(input_path).map_err(|e| e.to_string())?;
    let (w, h) = img.dimensions();

    let tile_w_raw = w as f64 / (cols as f64 - (cols as f64 - 1.0) * overlap_ratio);
    let tile_h_raw = h as f64 / (rows as f64 - (rows as f64 - 1.0) * overlap_ratio);

    let tile_w = tile_w_raw.ceil() as u32;
    let tile_h = tile_h_raw.ceil() as u32;
    let overlap_w = (tile_w as f64 * overlap_ratio) as u32;
    let overlap_h = (tile_h as f64 * overlap_ratio) as u32;

    let mut tiles = Vec::new();

    for r in 0..rows {
        for c in 0..cols {
            let x = c * (tile_w - overlap_w);
            let y = r * (tile_h - overlap_h);

            // Ensure we don't go out of bounds, though crop handles it usually
            // But we want consistent tile size if possible, or handle edge cases.
            // image::crop returns a view, converting to image copies it.
            let mut tile = img.crop_imm(x, y, tile_w, tile_h);

            // Save tile
            let file_name = format!("tile_{}_{}.png", r, c);
            let file_path = output_dir.join(&file_name);
            tile.to_image().save_with_format(&file_path, ImageFormat::Png).map_err(|e| e.to_string())?;

            tiles.push(TileInfo {
                r,
                c,
                x,
                y,
                width: tile.width(),
                height: tile.height(),
                path: file_path.to_string_lossy().to_string(),
            });
        }
    }

    Ok((tiles, w, h))
}

fn blend_images(img1: &RgbaImage, img2: &RgbaImage, overlap: u32, horizontal: bool) -> RgbaImage {
    let (w1, h1) = img1.dimensions();
    let (w2, h2) = img2.dimensions();

    if horizontal {
        let res_w = w1 + w2 - overlap;
        let res_h = h1.max(h2);
        let mut res = RgbaImage::from_pixel(res_w, res_h, Rgba([255, 255, 255, 255]));

        // Copy non-overlapping parts
        // Img1 left
        for y in 0..h1 {
            for x in 0..(w1 - overlap) {
                res.put_pixel(x, y, *img1.get_pixel(x, y));
            }
        }
        // Img2 right
        for y in 0..h2 {
            for x in 0..(w2 - overlap) { // overlap to end
                 res.put_pixel(w1 + x, y, *img2.get_pixel(overlap + x, y));
            }
        }

        // Blend overlap
        let common_h = h1.min(h2);
        for y in 0..common_h {
            for x in 0..overlap {
                let p1 = img1.get_pixel(w1 - overlap + x, y);
                let p2 = img2.get_pixel(x, y);

                let p1_white = is_white(p1);
                let p2_white = is_white(p2);

                let pixel = if p1_white && !p2_white {
                    *p2
                } else if !p1_white && p2_white {
                    *p1
                } else if !p1_white && !p2_white {
                    // Blend
                    let alpha = x as f32 / overlap as f32;
                    let r = ((1.0 - alpha) * p1[0] as f32 + alpha * p2[0] as f32) as u8;
                    let g = ((1.0 - alpha) * p1[1] as f32 + alpha * p2[1] as f32) as u8;
                    let b = ((1.0 - alpha) * p1[2] as f32 + alpha * p2[2] as f32) as u8;
                    // Use max alpha? Or average? 
                    // Usually alpha blending. Here inputs are opaque-ish.
                    Rgba([r, g, b, 255])
                } else {
                    Rgba([255, 255, 255, 255])
                };
                res.put_pixel(w1 - overlap + x, y, pixel);
            }
        }
        res
    } else {
        // Vertical
        let res_w = w1.max(w2);
        let res_h = h1 + h2 - overlap;
        let mut res = RgbaImage::from_pixel(res_w, res_h, Rgba([255, 255, 255, 255]));

        for y in 0..(h1 - overlap) {
            for x in 0..w1 {
                res.put_pixel(x, y, *img1.get_pixel(x, y));
            }
        }
        for y in 0..(h2 - overlap) {
            for x in 0..w2 {
                res.put_pixel(x, h1 + y, *img2.get_pixel(x, overlap + y));
            }
        }

        let common_w = w1.min(w2);
        for x in 0..common_w {
            for y in 0..overlap {
                let p1 = img1.get_pixel(x, h1 - overlap + y);
                let p2 = img2.get_pixel(x, y);

                let p1_white = is_white(p1);
                let p2_white = is_white(p2);

                let pixel = if p1_white && !p2_white {
                    *p2
                } else if !p1_white && p2_white {
                    *p1
                } else if !p1_white && !p2_white {
                    let alpha = y as f32 / overlap as f32;
                    let r = ((1.0 - alpha) * p1[0] as f32 + alpha * p2[0] as f32) as u8;
                    let g = ((1.0 - alpha) * p1[1] as f32 + alpha * p2[1] as f32) as u8;
                    let b = ((1.0 - alpha) * p1[2] as f32 + alpha * p2[2] as f32) as u8;
                    Rgba([r, g, b, 255])
                } else {
                    Rgba([255, 255, 255, 255])
                };
                res.put_pixel(x, h1 - overlap + y, pixel);
            }
        }
        res
    }
}
