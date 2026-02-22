use base64::{engine::general_purpose, Engine as _};
use exif::{In, Tag};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::{crop_imm, FilterType as ResizeFilterType};
use image::{ColorType, DynamicImage, ImageEncoder, Rgba, RgbaImage};
use rayon::prelude::*;
use std::io::{BufWriter, Cursor};
use std::path::Path;

fn open_image_with_orientation(path: &str) -> Result<DynamicImage, String> {
    let mut img = image::open(path).map_err(|e| e.to_string())?;

    // Try to read orientation from EXIF.
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

fn save_png_fast(path: &Path, image: &RgbaImage) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    let encoder =
        PngEncoder::new_with_quality(writer, CompressionType::Fast, PngFilterType::NoFilter);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug)]
enum ImageFileFormat {
    Png,
    Jpeg,
}

fn image_format_from_path(path: &Path) -> ImageFileFormat {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => ImageFileFormat::Jpeg,
        _ => ImageFileFormat::Png,
    }
}

fn file_extension(format: ImageFileFormat) -> &'static str {
    match format {
        ImageFileFormat::Png => "png",
        ImageFileFormat::Jpeg => "jpg",
    }
}

fn flatten_rgba_to_rgb_white(image: &RgbaImage) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((image.width() * image.height() * 3) as usize);
    for px in image.pixels() {
        let alpha = px[3] as u16;
        let inv_alpha = 255u16.saturating_sub(alpha);
        let r = ((px[0] as u16 * alpha + 255u16 * inv_alpha + 127) / 255) as u8;
        let g = ((px[1] as u16 * alpha + 255u16 * inv_alpha + 127) / 255) as u8;
        let b = ((px[2] as u16 * alpha + 255u16 * inv_alpha + 127) / 255) as u8;
        rgb.extend_from_slice(&[r, g, b]);
    }
    rgb
}

fn save_jpeg_fast(path: &Path, image: &RgbaImage, quality: u8) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    let mut encoder = JpegEncoder::new_with_quality(writer, quality);
    let rgb = flatten_rgba_to_rgb_white(image);
    encoder
        .encode(&rgb, image.width(), image.height(), ColorType::Rgb8)
        .map_err(|e| e.to_string())
}

fn save_image_fast(path: &Path, image: &RgbaImage, format: ImageFileFormat) -> Result<(), String> {
    match format {
        ImageFileFormat::Png => save_png_fast(path, image),
        ImageFileFormat::Jpeg => save_jpeg_fast(path, image, 90),
    }
}

fn save_image_fast_auto(path: &Path, image: &RgbaImage) -> Result<(), String> {
    save_image_fast(path, image, image_format_from_path(path))
}

fn encode_png_data_url_fast(image: &RgbaImage) -> Result<String, String> {
    let mut buffer = Cursor::new(Vec::new());
    let encoder =
        PngEncoder::new_with_quality(&mut buffer, CompressionType::Fast, PngFilterType::NoFilter);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;

    let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
    Ok(format!("data:image/png;base64,{}", b64))
}

fn encode_jpeg_data_url_fast(image: &RgbaImage, quality: u8) -> Result<String, String> {
    let mut buffer = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buffer, quality);
    let rgb = flatten_rgba_to_rgb_white(image);
    encoder
        .encode(&rgb, image.width(), image.height(), ColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
    Ok(format!("data:image/jpeg;base64,{}", b64))
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TileInfo {
    pub r: u32,
    pub c: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub path: String,
    pub original_path: String,
}

// Helper: Check if pixel matches key color.
fn is_key_color(p: &Rgba<u8>, color: &str, tolerance: u8) -> bool {
    if p[3] < 10 {
        return true;
    }

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
        _ => p[0] >= white_min && p[1] >= white_min && p[2] >= white_min,
    }
}

pub fn crop_image(
    input_path: &str,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    output_dir: &Path,
) -> Result<String, String> {
    let img = open_image_with_orientation(input_path)?.to_rgba8();
    let cropped = crop_imm(&img, x, y, width, height).to_image();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let file_name = format!("cropped_{}.png", timestamp);
    let file_path = output_dir.join(&file_name);

    save_png_fast(&file_path, &cropped)?;

    Ok(file_path.to_string_lossy().to_string())
}

pub fn save_resized_tile(path: &str, data: &[u8], width: u32, height: u32) -> Result<(), String> {
    let img = image::load_from_memory(data).map_err(|e| e.to_string())?;
    let resized = img
        .resize_exact(width, height, ResizeFilterType::Lanczos3)
        .to_rgba8();
    save_image_fast_auto(Path::new(path), &resized)
}

pub fn split_image(
    input_path: &str,
    rows: u32,
    cols: u32,
    overlap_ratio_x: f64,
    overlap_ratio_y: f64,
    prefer_jpeg: bool,
    output_dir: &Path,
) -> Result<(Vec<TileInfo>, u32, u32, String), String> {
    if rows == 0 || cols == 0 {
        return Err("Rows and cols must be greater than zero".to_string());
    }

    let img_rgba = open_image_with_orientation(input_path)?.to_rgba8();
    let (w, h) = img_rgba.dimensions();
    let image_format = if prefer_jpeg {
        ImageFileFormat::Jpeg
    } else {
        ImageFileFormat::Png
    };
    let ext = file_extension(image_format);

    // Save a copy of the original to the output_dir to ensure it survives temp dir replacement.
    let original_copy_path = output_dir.join(format!("original_source.{}", ext));
    save_image_fast(&original_copy_path, &img_rgba, image_format)?;
    let new_input_path = original_copy_path.to_string_lossy().to_string();

    let denom_w = cols as f64 - (cols as f64 - 1.0) * overlap_ratio_x;
    let denom_h = rows as f64 - (rows as f64 - 1.0) * overlap_ratio_y;
    if denom_w <= 0.0 || denom_h <= 0.0 {
        return Err("Invalid overlap/grid configuration".to_string());
    }

    let tile_w = (w as f64 / denom_w).ceil() as u32;
    let tile_h = (h as f64 / denom_h).ceil() as u32;
    let overlap_w = (tile_w as f64 * overlap_ratio_x) as u32;
    let overlap_h = (tile_h as f64 * overlap_ratio_y) as u32;
    let stride_w = tile_w.saturating_sub(overlap_w).max(1);
    let stride_h = tile_h.saturating_sub(overlap_h).max(1);

    let mut tile_configs = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        for c in 0..cols {
            let x = (c * stride_w).min(w.saturating_sub(1));
            let y = (r * stride_h).min(h.saturating_sub(1));
            let actual_w = tile_w.min(w - x);
            let actual_h = tile_h.min(h - y);
            if actual_w == 0 || actual_h == 0 {
                continue;
            }
            tile_configs.push((r, c, x, y, actual_w, actual_h));
        }
    }

    let tiles: Result<Vec<TileInfo>, String> = tile_configs
        .into_par_iter()
        .map(|(r, c, x, y, actual_w, actual_h)| {
            let tile = crop_imm(&img_rgba, x, y, actual_w, actual_h).to_image();

            let orig_file_name = format!("orig_tile_{}_{}.{}", r, c, ext);
            let orig_file_path = output_dir.join(&orig_file_name);
            save_image_fast(&orig_file_path, &tile, image_format)?;

            let proc_file_name = format!("tile_{}_{}.{}", r, c, ext);
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
        })
        .collect();

    Ok((tiles?, w, h, new_input_path))
}

pub fn merge_tiles(
    tile_paths: Vec<(u32, u32, String)>,
    original_w: u32,
    original_h: u32,
    overlap_ratio_x: f64,
    overlap_ratio_y: f64,
    key_color: &str,
    remove_bg: bool,
    tolerance: u8,
) -> Result<String, String> {
    if tile_paths.is_empty() {
        return Err("No tiles to merge".to_string());
    }
    if original_w == 0 || original_h == 0 {
        return Err("Invalid original image dimensions".to_string());
    }

    let max_r = tile_paths.iter().map(|(r, _, _)| *r).max().unwrap_or(0);
    let max_c = tile_paths.iter().map(|(_, c, _)| *c).max().unwrap_or(0);
    let rows = max_r + 1;
    let cols = max_c + 1;

    let denom_w = cols as f64 - (cols as f64 - 1.0) * overlap_ratio_x;
    let denom_h = rows as f64 - (rows as f64 - 1.0) * overlap_ratio_y;
    if denom_w <= 0.0 || denom_h <= 0.0 {
        return Err("Invalid overlap/grid configuration".to_string());
    }

    let tile_w = (original_w as f64 / denom_w).ceil() as u32;
    let tile_h = (original_h as f64 / denom_h).ceil() as u32;
    let overlap_w = (tile_w as f64 * overlap_ratio_x) as u32;
    let overlap_h = (tile_h as f64 * overlap_ratio_y) as u32;
    let stride_w = tile_w.saturating_sub(overlap_w).max(1);
    let stride_h = tile_h.saturating_sub(overlap_h).max(1);

    #[derive(Debug)]
    struct TileJob {
        r: u32,
        c: u32,
        start_x: u32,
        start_y: u32,
        expected_w: u32,
        expected_h: u32,
        path: String,
    }

    #[derive(Debug)]
    struct LoadedTile {
        r: u32,
        c: u32,
        start_x: u32,
        start_y: u32,
        image: RgbaImage,
    }

    let jobs: Vec<TileJob> = tile_paths
        .into_iter()
        .filter_map(|(r, c, path)| {
            let start_x = (c * stride_w).min(original_w.saturating_sub(1));
            let start_y = (r * stride_h).min(original_h.saturating_sub(1));

            let expected_w = tile_w.min(original_w - start_x);
            let expected_h = tile_h.min(original_h - start_y);
            if expected_w == 0 || expected_h == 0 {
                return None;
            }

            Some(TileJob {
                r,
                c,
                start_x,
                start_y,
                expected_w,
                expected_h,
                path,
            })
        })
        .collect();

    if jobs.is_empty() {
        return Err("No valid tiles to merge".to_string());
    }

    let mut loaded_tiles: Vec<LoadedTile> = jobs
        .into_par_iter()
        .map(|job| {
            let mut final_path = job.path.clone();
            if !Path::new(&job.path).exists() {
                let dir = Path::new(&job.path).parent().ok_or("Invalid tile path")?;
                let mut fallback_path = None;
                for ext in ["png", "jpg", "jpeg"] {
                    let candidate = dir.join(format!("orig_tile_{}_{}.{}", job.r, job.c, ext));
                    if candidate.exists() {
                        fallback_path = Some(candidate);
                        break;
                    }
                }
                if let Some(fallback) = fallback_path {
                    final_path = fallback.to_string_lossy().to_string();
                } else {
                    return Err(format!(
                        "Tile result and original both missing for {},{}",
                        job.r, job.c
                    ));
                }
            }

            let mut img = image::open(&final_path)
                .map_err(|e| format!("Failed to open {}: {}", final_path, e))?
                .to_rgba8();

            if img.width() != job.expected_w || img.height() != job.expected_h {
                img = DynamicImage::ImageRgba8(img)
                    .resize_exact(job.expected_w, job.expected_h, ResizeFilterType::Lanczos3)
                    .to_rgba8();
            }

            Ok(LoadedTile {
                r: job.r,
                c: job.c,
                start_x: job.start_x,
                start_y: job.start_y,
                image: img,
            })
        })
        .collect::<Result<_, String>>()?;

    loaded_tiles.sort_unstable_by_key(|t| (t.r, t.c));

    let mut final_img = RgbaImage::new(original_w, original_h);
    let final_stride = original_w as usize * 4;
    let x_ramp: Vec<f32> = if overlap_w > 0 {
        (0..overlap_w)
            .map(|x| x as f32 / overlap_w as f32)
            .collect()
    } else {
        Vec::new()
    };
    let y_ramp: Vec<f32> = if overlap_h > 0 {
        (0..overlap_h)
            .map(|y| y as f32 / overlap_h as f32)
            .collect()
    } else {
        Vec::new()
    };

    {
        let mut final_samples = final_img.as_flat_samples_mut();
        let final_raw = final_samples.as_mut_slice();

        for tile in &loaded_tiles {
            let tile_w = tile.image.width();
            let tile_h = tile.image.height();
            let tile_stride = tile_w as usize * 4;
            let tile_raw = tile.image.as_raw();

            let has_left = tile.c > 0 && overlap_w > 0;
            let has_top = tile.r > 0 && overlap_h > 0;

            for y in 0..tile_h {
                let global_y = tile.start_y + y;
                if global_y >= original_h {
                    continue;
                }

                let in_overlap_y = has_top && y < overlap_h;
                let y_factor = if in_overlap_y {
                    y_ramp[y as usize]
                } else {
                    1.0
                };
                let tile_row_offset = y as usize * tile_stride;
                let final_row_offset = global_y as usize * final_stride;

                for x in 0..tile_w {
                    let global_x = tile.start_x + x;
                    if global_x >= original_w {
                        continue;
                    }

                    let in_overlap_x = has_left && x < overlap_w;
                    let src_idx = tile_row_offset + x as usize * 4;
                    let dst_idx = final_row_offset + global_x as usize * 4;

                    let new_px = [
                        tile_raw[src_idx],
                        tile_raw[src_idx + 1],
                        tile_raw[src_idx + 2],
                        tile_raw[src_idx + 3],
                    ];

                    if !in_overlap_x && !in_overlap_y {
                        final_raw[dst_idx] = new_px[0];
                        final_raw[dst_idx + 1] = new_px[1];
                        final_raw[dst_idx + 2] = new_px[2];
                        final_raw[dst_idx + 3] = new_px[3];
                        continue;
                    }

                    let old_px = [
                        final_raw[dst_idx],
                        final_raw[dst_idx + 1],
                        final_raw[dst_idx + 2],
                        final_raw[dst_idx + 3],
                    ];

                    if old_px[3] == 0 {
                        final_raw[dst_idx] = new_px[0];
                        final_raw[dst_idx + 1] = new_px[1];
                        final_raw[dst_idx + 2] = new_px[2];
                        final_raw[dst_idx + 3] = new_px[3];
                        continue;
                    }

                    if remove_bg {
                        let p_new = Rgba(new_px);
                        let p_old = Rgba(old_px);
                        let p_new_key = is_key_color(&p_new, key_color, tolerance);
                        let p_old_key = is_key_color(&p_old, key_color, tolerance);

                        if p_new_key && !p_old_key {
                            continue;
                        }
                        if !p_new_key && p_old_key {
                            final_raw[dst_idx] = new_px[0];
                            final_raw[dst_idx + 1] = new_px[1];
                            final_raw[dst_idx + 2] = new_px[2];
                            final_raw[dst_idx + 3] = new_px[3];
                            continue;
                        }
                    }

                    let factor = if in_overlap_x && !in_overlap_y {
                        x_ramp[x as usize]
                    } else if in_overlap_y && !in_overlap_x {
                        y_factor
                    } else {
                        x_ramp[x as usize].max(y_factor)
                    };

                    if factor <= 0.0 {
                        continue;
                    }
                    if factor >= 1.0 {
                        final_raw[dst_idx] = new_px[0];
                        final_raw[dst_idx + 1] = new_px[1];
                        final_raw[dst_idx + 2] = new_px[2];
                        final_raw[dst_idx + 3] = new_px[3];
                        continue;
                    }

                    let inv = 1.0 - factor;
                    final_raw[dst_idx] = (inv * old_px[0] as f32 + factor * new_px[0] as f32) as u8;
                    final_raw[dst_idx + 1] =
                        (inv * old_px[1] as f32 + factor * new_px[1] as f32) as u8;
                    final_raw[dst_idx + 2] =
                        (inv * old_px[2] as f32 + factor * new_px[2] as f32) as u8;
                    final_raw[dst_idx + 3] =
                        (inv * old_px[3] as f32 + factor * new_px[3] as f32) as u8;
                }
            }
        }
    }

    if remove_bg {
        final_img
            .as_flat_samples_mut()
            .as_mut_slice()
            .par_chunks_exact_mut(4)
            .for_each(|pixel| {
                let p = Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]);
                if is_key_color(&p, key_color, tolerance) {
                    pixel[0] = 0;
                    pixel[1] = 0;
                    pixel[2] = 0;
                    pixel[3] = 0;
                }
            });
    }

    if remove_bg {
        encode_png_data_url_fast(&final_img)
    } else {
        encode_jpeg_data_url_fast(&final_img, 90)
    }
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
