use image::{GenericImageView, ImageFormat, Rgba, RgbaImage, imageops::FilterType};
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
    pub path: String,           // Path for the processed tile
    pub original_path: String,  // Path for the original source tile
}

// Helper: Check if pixel matches key color
fn is_key_color(p: &Rgba<u8>, color: &str, tolerance: u8) -> bool {
    if p[3] < 10 {
        return true;
    }
    
    // Default strict thresholds
    let white_min = 240u8.saturating_sub(tolerance);
    let black_max = 15u8.saturating_add(tolerance);
    
    let color_min = 240u8.saturating_sub(tolerance);
    let color_max = 50u8.saturating_add(tolerance);

    match color {
        "white" => p[0] >= white_min && p[1] >= white_min && p[2] >= white_min,
        "black" => p[0] <= black_max && p[1] <= black_max && p[2] <= black_max,
        "red" => p[0] >= color_min && p[1] <= color_max && p[2] <= color_max,
        "blue" => p[0] <= color_max && p[1] <= color_max && p[2] >= color_min,
        "green" => p[0] <= color_max && p[1] >= color_min && p[2] <= color_max,
        _ => p[0] >= white_min && p[1] >= white_min && p[2] >= white_min, // Default white
    }
}

pub fn crop_image(input_path: &str, x: u32, y: u32, width: u32, height: u32, output_dir: &Path) -> Result<String, String> {
    let img = image::open(input_path).map_err(|e| e.to_string())?;
    let cropped = img.crop_imm(x, y, width, height);
    
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let file_name = format!("cropped_{}.png", timestamp);
    let file_path = output_dir.join(&file_name);
    
    cropped.to_rgba8().save_with_format(&file_path, ImageFormat::Png).map_err(|e| e.to_string())?;
    
    Ok(file_path.to_string_lossy().to_string())
}

pub fn save_resized_tile(path: &str, data: &[u8], width: u32, height: u32) -> Result<(), String> {
    let img = image::load_from_memory(data).map_err(|e| e.to_string())?;
    let resized = img.resize_exact(width, height, FilterType::Lanczos3);
    resized.save_with_format(path, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(())
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

            // Ensure we don't exceed image dimensions
            let actual_w = tile_w.min(w - x);
            let actual_h = tile_h.min(h - y);

            let tile = img.crop_imm(x, y, actual_w, actual_h);

            // Save original tile
            let orig_file_name = format!("orig_tile_{}_{}.png", r, c);
            let orig_file_path = output_dir.join(&orig_file_name);
            tile.to_rgba8().save_with_format(&orig_file_path, ImageFormat::Png).map_err(|e| e.to_string())?;

            // Prepare path for processed tile result
            let proc_file_name = format!("tile_{}_{}.png", r, c);
            let proc_file_path = output_dir.join(&proc_file_name);

            tiles.push(TileInfo {
                r,
                c,
                x,
                y,
                width: tile.width(),
                height: tile.height(),
                path: proc_file_path.to_string_lossy().to_string(),
                original_path: orig_file_path.to_string_lossy().to_string(),
            });
        }
    }

    Ok((tiles, w, h))
}

pub fn merge_tiles(
    tile_paths: Vec<(u32, u32, String)>, // (row, col, path)
    _original_w: u32,
    _original_h: u32,
    overlap_ratio: f64,
    key_color: &str,
    remove_bg: bool,
    tolerance: u8,
) -> Result<String, String> {
    let max_r = tile_paths.iter().map(|(r, _, _)| *r).max().unwrap_or(0);
    let max_c = tile_paths.iter().map(|(_, c, _)| *c).max().unwrap_or(0);
    let rows = max_r + 1;
    let cols = max_c + 1;

    let mut tile_map = std::collections::HashMap::new();
    let mut first_tile_w = 0;
    let mut first_tile_h = 0;

    for (r, c, path) in tile_paths {
        let mut final_path = path.clone();
        if !std::path::Path::new(&path).exists() {
            let dir = std::path::Path::new(&path).parent().unwrap();
            let orig_path = dir.join(format!("orig_tile_{}_{}.png", r, c));
            if orig_path.exists() {
                final_path = orig_path.to_string_lossy().to_string();
            }
        }

        let img = image::open(&final_path).map_err(|e| format!("Failed to open {}: {}", final_path, e))?.to_rgba8();
        if r == 0 && c == 0 {
            first_tile_w = img.width();
            first_tile_h = img.height();
        }
        tile_map.insert((r, c), img);
    }

    let overlap_w = (first_tile_w as f64 * overlap_ratio) as u32;
    let overlap_h = (first_tile_h as f64 * overlap_ratio) as u32;

    let mut row_imgs = Vec::new();
    for r in 0..rows {
        let mut row_img = tile_map.get(&(r, 0)).ok_or(format!("Missing tile {},0", r))?.clone();
        for c in 1..cols {
            let next_tile = tile_map.get(&(r, c)).ok_or(format!("Missing tile {},{}", r, c))?;
            row_img = blend_images(&row_img, next_tile, overlap_w, true, key_color, tolerance);
        }
        row_imgs.push(row_img);
    }

    let mut final_img = row_imgs[0].clone();
    for r in 1..rows {
        final_img = blend_images(&final_img, &row_imgs[r as usize], overlap_h, false, key_color, tolerance);
    }

    if remove_bg {
        for pixel in final_img.pixels_mut() {
            if is_key_color(pixel, key_color, tolerance) {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
    }

    let mut buffer = Cursor::new(Vec::new());
    final_img.write_to(&mut buffer, ImageFormat::Png).map_err(|e| e.to_string())?;
    
    let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
    Ok(format!("data:image/png;base64,{}", b64))
}

fn blend_images(img1: &RgbaImage, img2: &RgbaImage, overlap: u32, horizontal: bool, key_color: &str, tolerance: u8) -> RgbaImage {
    let (w1, h1) = img1.dimensions();
    let (w2, h2) = img2.dimensions();

    if horizontal {
        let res_w = w1 + w2 - overlap;
        let res_h = h1.max(h2);
        let mut res = RgbaImage::from_pixel(res_w, res_h, Rgba([255, 255, 255, 255]));

        for y in 0..h1 {
            for x in 0..(w1 - overlap) {
                res.put_pixel(x, y, *img1.get_pixel(x, y));
            }
        }
        for y in 0..h2 {
            for x in 0..(w2 - overlap) {
                 res.put_pixel(w1 + x, y, *img2.get_pixel(overlap + x, y));
            }
        }

        let common_h = h1.min(h2);
        for y in 0..common_h {
            for x in 0..overlap {
                let p1 = img1.get_pixel(w1 - overlap + x, y);
                let p2 = img2.get_pixel(x, y);

                let p1_key = is_key_color(p1, key_color, tolerance);
                let p2_key = is_key_color(p2, key_color, tolerance);

                let pixel = if p1_key && !p2_key {
                    *p2
                } else if !p1_key && p2_key {
                    *p1
                } else if !p1_key && !p2_key {
                    let alpha = x as f32 / overlap as f32;
                    let r = ((1.0 - alpha) * p1[0] as f32 + alpha * p2[0] as f32) as u8;
                    let g = ((1.0 - alpha) * p1[1] as f32 + alpha * p2[1] as f32) as u8;
                    let b = ((1.0 - alpha) * p1[2] as f32 + alpha * p2[2] as f32) as u8;
                    let a = ((1.0 - alpha) * p1[3] as f32 + alpha * p2[3] as f32) as u8;
                    Rgba([r, g, b, a])
                } else {
                    // Both are key color, use key color or transparent?
                    // For now, use p2.
                    *p2
                };
                res.put_pixel(w1 - overlap + x, y, pixel);
            }
        }
        res
    } else {
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

                let p1_key = is_key_color(p1, key_color, tolerance);
                let p2_key = is_key_color(p2, key_color, tolerance);

                let pixel = if p1_key && !p2_key {
                    *p2
                } else if !p1_key && p2_key {
                    *p1
                } else if !p1_key && !p2_key {
                    let alpha = y as f32 / overlap as f32;
                    let r = ((1.0 - alpha) * p1[0] as f32 + alpha * p2[0] as f32) as u8;
                    let g = ((1.0 - alpha) * p1[1] as f32 + alpha * p2[1] as f32) as u8;
                    let b = ((1.0 - alpha) * p1[2] as f32 + alpha * p2[2] as f32) as u8;
                    let a = ((1.0 - alpha) * p1[3] as f32 + alpha * p2[3] as f32) as u8;
                    Rgba([r, g, b, a])
                } else {
                    *p2
                };
                res.put_pixel(x, h1 - overlap + y, pixel);
            }
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_white() {
        assert!(is_white(&Rgba([255, 255, 255, 255])));
        assert!(is_white(&Rgba([0, 0, 0, 0]))); 
        assert!(!is_white(&Rgba([255, 0, 0, 255]))); 
    }
}
