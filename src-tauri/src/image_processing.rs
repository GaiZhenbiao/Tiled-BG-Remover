use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage, imageops::FilterType};
use std::path::Path;
use base64::{Engine as _, engine::general_purpose};
use std::io::Cursor;
use rayon::prelude::*;
use exif::{In, Tag};

fn open_image_with_orientation(path: &str) -> Result<DynamicImage, String> {
    let mut img = image::open(path).map_err(|e| e.to_string())?;

    // Try to read orientation from EXIF
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut reader = std::io::BufReader::new(file);
    if let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) {
        if let Some(field) = exif.get_field(Tag::Orientation, In::PRIMARY) {
            if let Some(orientation) = field.value.get_uint(0) {
                img = match orientation {
                    2 => img.fliph(),
                    3 => img.rotate180(),
                    4 => img.flipv(),
                    5 => img.rotate90().fliph(),
                    6 => img.rotate90(),
                    7 => img.rotate270().fliph(),
                    8 => img.rotate270(),
                    _ => img,
                };
            }
        }
    }

    Ok(img)
}

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
    let img = open_image_with_orientation(input_path)?;
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
) -> Result<(Vec<TileInfo>, u32, u32, String), String> {
    let img = open_image_with_orientation(input_path)?;
    let (w, h) = img.dimensions();

    // Save a copy of the original to the output_dir to ensure it survives temp dir replacement
    let original_copy_path = output_dir.join("original_source.png");
    img.save_with_format(&original_copy_path, ImageFormat::Png).map_err(|e| e.to_string())?;
    let new_input_path = original_copy_path.to_string_lossy().to_string();

    let img_rgba = img.to_rgba8();

    let tile_w_raw = w as f64 / (cols as f64 - (cols as f64 - 1.0) * overlap_ratio);
    let tile_h_raw = h as f64 / (rows as f64 - (rows as f64 - 1.0) * overlap_ratio);

    let tile_w = tile_w_raw.ceil() as u32;
    let tile_h = tile_h_raw.ceil() as u32;
    let overlap_w = (tile_w as f64 * overlap_ratio) as u32;
    let overlap_h = (tile_h as f64 * overlap_ratio) as u32;

    let mut tile_configs = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let x = c * (tile_w - overlap_w);
            let y = r * (tile_h - overlap_h);
            let actual_w = tile_w.min(w - x);
            let actual_h = tile_h.min(h - y);
            tile_configs.push((r, c, x, y, actual_w, actual_h));
        }
    }

    let tiles: Result<Vec<TileInfo>, String> = tile_configs.into_par_iter().map(|(r, c, x, y, actual_w, actual_h)| {
        let tile = img.view(x, y, actual_w, actual_h).to_image();

        // Save original tile
        let orig_file_name = format!("orig_tile_{}_{}.png", r, c);
        let orig_file_path = output_dir.join(&orig_file_name);
        tile.save_with_format(&orig_file_path, ImageFormat::Png).map_err(|e| e.to_string())?;

        // Prepare path for processed tile result
        let proc_file_name = format!("tile_{}_{}.png", r, c);
        let proc_file_path = output_dir.join(&proc_file_name);

        Ok(TileInfo {
            r,
            c,
            x,
            y,
            width: actual_w,
            height: actual_h,
            path: proc_file_path.to_string_lossy().to_string(),
            original_path: orig_file_path.to_string_lossy().to_string(),
        })
    }).collect();

    Ok((tiles?, w, h, new_input_path))
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
    if tile_paths.is_empty() {
        return Err("No tiles to merge".to_string());
    }

    let max_r = tile_paths.iter().map(|(r, _, _)| *r).max().unwrap_or(0);
    let max_c = tile_paths.iter().map(|(_, c, _)| *c).max().unwrap_or(0);
    let rows = max_r + 1;
    let cols = max_c + 1;

    // Load tiles in parallel
    let loaded_tiles: Result<std::collections::HashMap<(u32, u32), RgbaImage>, String> = tile_paths.into_par_iter().map(|(r, c, path)| {
        let mut final_path = path.clone();
        if !std::path::Path::new(&path).exists() {
            let dir = std::path::Path::new(&path).parent().ok_or("Invalid path")?;
            let orig_path = dir.join(format!("orig_tile_{}_{}.png", r, c));
            if orig_path.exists() {
                final_path = orig_path.to_string_lossy().to_string();
            } else {
                return Err(format!("Tile result and original both missing for {},{}", r, c));
            }
        }

        let img = image::open(&final_path).map_err(|e| format!("Failed to open {}: {}", final_path, e))?.to_rgba8();
        Ok(((r, c), img))
    }).collect();

    let tile_map = loaded_tiles?;
    
    // Determine dimensions from tiles
    let (first_tile_w, first_tile_h) = tile_map.get(&(0, 0)).map(|img| img.dimensions()).ok_or("Missing tile 0,0")?;
    let overlap_w = (first_tile_w as f64 * overlap_ratio) as u32;
    let overlap_h = (first_tile_h as f64 * overlap_ratio) as u32;

    let stride_w = first_tile_w - overlap_w;
    let stride_h = first_tile_h - overlap_h;

    // Calculate final image size
    // Final width = (cols - 1) * stride_w + last_tile_width
    let last_tile_w = tile_map.get(&(0, max_c)).map(|img| img.width()).unwrap_or(first_tile_w);
    let last_tile_h = tile_map.get(&(max_r, 0)).map(|img| img.height()).unwrap_or(first_tile_h);
    
    let res_w = max_c * stride_w + last_tile_w;
    let res_h = max_r * stride_h + last_tile_h;

    let mut final_img = RgbaImage::new(res_w, res_h);

    // Instead of chaining blends, we iterate over each pixel of the result and calculate its value.
    // This is more complex but more efficient and easier to parallelize.
    // For simplicity, let's just do it sequentially but avoid multiple image allocations.
    
    for r in 0..rows {
        for c in 0..cols {
            if let Some(tile) = tile_map.get(&(r, c)) {
                let start_x = c * stride_w;
                let start_y = r * stride_h;
                
                for y in 0..tile.height() {
                    for x in 0..tile.width() {
                        let global_x = start_x + x;
                        let global_y = start_y + y;
                        
                        if global_x >= res_w || global_y >= res_h { continue; }
                        
                        let p_new = tile.get_pixel(x, y);
                        
                        // If it's the first time we touch this pixel (no overlap from previous tiles in top/left)
                        // Or if we are in an overlap region, we need to blend.
                        
                        // Simplified blending: 
                        // If it's a first-seen pixel, just put it.
                        // If it's an overlap, blend with what's already there.
                        
                        // We process tiles from top-left to bottom-right.
                        // Overlap can happen with:
                        // - Left tile (c-1) if x < overlap_w
                        // - Top tile (r-1) if y < overlap_h
                        // - Top-left tile (r-1, c-1) if x < overlap_w && y < overlap_h
                        
                        let in_overlap_x = c > 0 && x < overlap_w;
                        let in_overlap_y = r > 0 && y < overlap_h;
                        
                        if !in_overlap_x && !in_overlap_y {
                            final_img.put_pixel(global_x, global_y, *p_new);
                        } else {
                            let p_old = final_img.get_pixel(global_x, global_y);
                            
                            let p_new_key = is_key_color(p_new, key_color, tolerance);
                            let p_old_key = is_key_color(p_old, key_color, tolerance);
                            
                            let blended = if p_new_key && !p_old_key {
                                *p_old
                            } else if !p_new_key && p_old_key {
                                *p_new
                            } else if !p_new_key && !p_old_key {
                                // Linear blend based on distance to edge
                                // If overlap in both, we should ideally handle 4-way blend, 
                                // but 2-way at a time (as we go tile by tile) might be enough.
                                
                                let factor = if in_overlap_x && !in_overlap_y {
                                    x as f32 / overlap_w as f32
                                } else if in_overlap_y && !in_overlap_x {
                                    y as f32 / overlap_h as f32
                                } else {
                                    // Corner: average of both factors?
                                    (x as f32 / overlap_w as f32).max(y as f32 / overlap_h as f32)
                                };
                                
                                let r = ((1.0 - factor) * p_old[0] as f32 + factor * p_new[0] as f32) as u8;
                                let g = ((1.0 - factor) * p_old[1] as f32 + factor * p_new[1] as f32) as u8;
                                let b = ((1.0 - factor) * p_old[2] as f32 + factor * p_new[2] as f32) as u8;
                                let a = ((1.0 - factor) * p_old[3] as f32 + factor * p_new[3] as f32) as u8;
                                Rgba([r, g, b, a])
                            } else {
                                *p_new
                            };
                            final_img.put_pixel(global_x, global_y, blended);
                        }
                    }
                }
            }
        }
    }

    if remove_bg {
        final_img.as_flat_samples_mut().as_mut_slice().par_chunks_exact_mut(4).for_each(|pixel| {
            let p = Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]);
            if is_key_color(&p, key_color, tolerance) {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
                pixel[3] = 0;
            }
        });
    }

    let mut buffer = Cursor::new(Vec::new());
    final_img.write_to(&mut buffer, ImageFormat::Png).map_err(|e| e.to_string())?;
    
    let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
    Ok(format!("data:image/png;base64,{}", b64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_key_color() {
        assert!(is_key_color(&Rgba([255, 255, 255, 255]), "white", 10));
        assert!(is_key_color(&Rgba([0, 0, 0, 0]), "white", 10)); 
        assert!(!is_key_color(&Rgba([255, 0, 0, 255]), "white", 10)); 
    }
}
