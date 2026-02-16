use image::{GenericImageView, ImageFormat, Rgba, RgbaImage};
use std::path::Path;
use base64::{Engine as _, engine::general_purpose};
use std::io::Cursor;

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
    
    // Fix: Use to_rgba8() instead of to_image()
    cropped.to_rgba8().save_with_format(&file_path, ImageFormat::Png).map_err(|e| e.to_string())?;
    
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
            // Fix: Use to_rgba8() instead of to_image()
            tile.to_rgba8().save_with_format(&file_path, ImageFormat::Png).map_err(|e| e.to_string())?;

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

pub fn merge_tiles(
    tile_paths: Vec<(u32, u32, String)>, // (row, col, path)
    original_w: u32,
    original_h: u32,
    overlap_ratio: f64,
) -> Result<String, String> {
    // Determine grid size
    let max_r = tile_paths.iter().map(|(r, _, _)| *r).max().unwrap_or(0);
    let max_c = tile_paths.iter().map(|(_, c, _)| *c).max().unwrap_or(0);
    let rows = max_r + 1;
    let cols = max_c + 1;

    // Load all tiles
    let mut tile_map = std::collections::HashMap::new();
    let mut first_tile_w = 0;
    let mut first_tile_h = 0;

    for (r, c, path) in tile_paths {
        let img = image::open(&path).map_err(|e| format!("Failed to open {}: {}", path, e))?.to_rgba8();
        if r == 0 && c == 0 {
            first_tile_w = img.width();
            first_tile_h = img.height();
        }
        tile_map.insert((r, c), img);
    }

    let overlap_w = (first_tile_w as f64 * overlap_ratio) as u32;
    let overlap_h = (first_tile_h as f64 * overlap_ratio) as u32;

    // Stitch rows
    let mut row_imgs = Vec::new();
    for r in 0..rows {
        let mut row_img = tile_map.get(&(r, 0)).ok_or(format!("Missing tile {},0", r))?.clone();
        for c in 1..cols {
            let next_tile = tile_map.get(&(r, c)).ok_or(format!("Missing tile {},{}", r, c))?;
            row_img = blend_images(&row_img, next_tile, overlap_w, true);
        }
        row_imgs.push(row_img);
    }

    // Stitch cols
    let mut final_img = row_imgs[0].clone();
    for r in 1..rows {
        final_img = blend_images(&final_img, &row_imgs[r as usize], overlap_h, false);
    }

    // Convert to base64 to return
    let mut buffer = Cursor::new(Vec::new());
    final_img.write_to(&mut buffer, ImageFormat::Png).map_err(|e| e.to_string())?;
    
    let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
    Ok(format!("data:image/png;base64,{}", b64))
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