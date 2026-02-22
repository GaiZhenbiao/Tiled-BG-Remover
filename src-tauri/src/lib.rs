use base64::{engine::general_purpose, Engine as _};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::{ColorType, DynamicImage, ImageEncoder, RgbaImage};
use psd_rs::{Document, Layer};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;

mod image_processing;
use image_processing::{merge_tiles, split_image, TileInfo};

// State to hold temp directory
struct AppState {
    temp_dir: Mutex<Option<TempDir>>,
}

#[derive(serde::Serialize)]
struct SplitResponse {
    tiles: Vec<TileInfo>,
    original_width: u32,
    original_height: u32,
    temp_dir: String,
    new_input_path: String,
}

#[tauri::command]
fn load_image(path: String) -> Result<String, String> {
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let b64 = general_purpose::STANDARD.encode(&data);
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png");
    let mime = match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
fn save_image(path: String, base64_data: String) -> Result<(), String> {
    let data_str = base64_data.split(',').last().unwrap_or(&base64_data);
    let data = general_purpose::STANDARD
        .decode(data_str)
        .map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_image_resized(
    path: String,
    base64_data: String,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let data_str = base64_data.split(',').last().unwrap_or(&base64_data);
    let data = general_purpose::STANDARD
        .decode(data_str)
        .map_err(|e| e.to_string())?;
    image_processing::save_resized_tile(&path, &data, width, height)
}

#[tauri::command]
async fn split_img(
    state: tauri::State<'_, AppState>,
    path: String,
    rows: u32,
    cols: u32,
    overlap_ratio_x: f64,
    overlap_ratio_y: f64,
    prefer_jpeg: bool,
) -> Result<SplitResponse, String> {
    let path_clone = path.clone();

    // Offload heavy image processing to a blocking thread
    let result = tauri::async_runtime::spawn_blocking(move || {
        let td = TempDir::new().map_err(|e| e.to_string())?;
        let td_path = td.path().to_path_buf();

        let (tiles, w, h, new_path) = split_image(
            &path_clone,
            rows,
            cols,
            overlap_ratio_x,
            overlap_ratio_y,
            prefer_jpeg,
            &td_path,
        )?;

        Ok::<_, String>((td, tiles, w, h, td_path, new_path))
    })
    .await
    .map_err(|e| e.to_string())??;

    let (td, tiles, w, h, td_path_buf, new_path) = result;

    let mut state_temp = state
        .temp_dir
        .lock()
        .map_err(|_| "Failed to lock state".to_string())?;
    *state_temp = Some(td);

    Ok(SplitResponse {
        tiles,
        original_width: w,
        original_height: h,
        temp_dir: td_path_buf.to_string_lossy().to_string(),
        new_input_path: new_path,
    })
}

#[derive(serde::Deserialize)]
struct TileUpdate {
    r: u32,
    c: u32,
    path: String,
}

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExportTile {
    r: u32,
    c: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    path: String,
    original_path: String,
}

#[derive(serde::Serialize)]
struct SaveBundleResponse {
    export_dir: String,
    merged_path: String,
    psd_path: String,
    tiles_dir: String,
    tile_count: usize,
    image_format: String,
    psd_logs: Vec<String>,
}

struct LayerExport {
    tile: ExportTile,
    image: RgbaImage,
}

#[derive(Clone, Copy)]
enum ExportImageFormat {
    Png,
    Jpeg,
}

impl ExportImageFormat {
    fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }
}

fn decode_data_url(data: &str) -> Result<Vec<u8>, String> {
    let payload = data.split(',').last().unwrap_or(data);
    general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| e.to_string())
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

fn save_png_fast(path: &Path, image: &RgbaImage) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
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

fn save_jpeg_fast(path: &Path, image: &RgbaImage, quality: u8) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    let mut encoder = JpegEncoder::new_with_quality(writer, quality);
    let rgb = flatten_rgba_to_rgb_white(image);
    encoder
        .encode(&rgb, image.width(), image.height(), ColorType::Rgb8)
        .map_err(|e| e.to_string())
}

fn write_image_with_format(
    path: &Path,
    image: &RgbaImage,
    format: ExportImageFormat,
) -> Result<(), String> {
    match format {
        ExportImageFormat::Png => save_png_fast(path, image),
        ExportImageFormat::Jpeg => save_jpeg_fast(path, image, 90),
    }
}

fn resolve_tile_source(tile: &ExportTile) -> Result<PathBuf, String> {
    let processed = tile.path.trim();
    if !processed.is_empty() {
        let processed_path = PathBuf::from(processed);
        if processed_path.is_file() {
            return Ok(processed_path);
        }
    }

    let original = tile.original_path.trim();
    if !original.is_empty() {
        let original_path = PathBuf::from(original);
        if original_path.is_file() {
            return Ok(original_path);
        }
    }

    Err(format!(
        "Missing tile files for {},{} (processed: {}, original: {})",
        tile.r, tile.c, tile.path, tile.original_path
    ))
}

fn resolve_input_source_path(input_path: Option<&str>, tiles: &[ExportTile]) -> Option<PathBuf> {
    if let Some(path) = input_path.map(str::trim).filter(|p| !p.is_empty()) {
        let direct_path = PathBuf::from(path);
        if direct_path.is_file() {
            return Some(direct_path);
        }
    }

    for tile in tiles {
        let original = tile.original_path.trim();
        if original.is_empty() {
            continue;
        }

        let original_path = PathBuf::from(original);
        if let Some(parent) = original_path.parent() {
            for ext in ["png", "jpg", "jpeg"] {
                let candidate = parent.join(format!("original_source.{}", ext));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn sanitize_path_component(input: &str) -> String {
    let mut out: String = input
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();

    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        "image".to_string()
    } else {
        out
    }
}

fn append_psd_log(psd_logs: &mut Vec<String>, verbose_logging: bool, message: &str) {
    if verbose_logging {
        psd_logs.push(format!("[PSD] {}", message));
    }
}

fn write_psd(
    psd_path: &Path,
    source_image: &RgbaImage,
    merged_image: &RgbaImage,
    layers: &[LayerExport],
    psd_logs: &mut Vec<String>,
    verbose_logging: bool,
) -> Result<(), String> {
    append_psd_log(
        psd_logs,
        verbose_logging,
        &format!(
            "start: os={}, target={}, source={}x{}, merged={}x{}, tiles={}",
            std::env::consts::OS,
            psd_path.to_string_lossy(),
            source_image.width(),
            source_image.height(),
            merged_image.width(),
            merged_image.height(),
            layers.len()
        ),
    );

    let mut document = Document::new();

    let mut input_layer = Layer::new("Input Source");
    input_layer
        .set_image(
            source_image.as_raw(),
            source_image.height() as usize,
            source_image.width() as usize,
        )
        .map_err(|e| e.to_string())?;
    input_layer.set_offset(0, 0);
    document.push(input_layer).map_err(|e| e.to_string())?;

    let mut merged_layer = Layer::new("Merged Result");
    merged_layer
        .set_image(
            merged_image.as_raw(),
            merged_image.height() as usize,
            merged_image.width() as usize,
        )
        .map_err(|e| e.to_string())?;
    merged_layer.set_offset(0, 0);
    document.push(merged_layer).map_err(|e| e.to_string())?;

    for layer in layers {
        let mut psd_layer = Layer::new(format!("Tile r{} c{}", layer.tile.r, layer.tile.c));
        psd_layer
            .set_image(
                layer.image.as_raw(),
                layer.image.height() as usize,
                layer.image.width() as usize,
            )
            .map_err(|e| e.to_string())?;
        psd_layer.set_offset(layer.tile.x as usize, layer.tile.y as usize);
        document.push(psd_layer).map_err(|e| e.to_string())?;
    }

    let target = psd_path.to_string_lossy().to_string();
    match document.save(&target) {
        Ok(_) => {
            append_psd_log(psd_logs, verbose_logging, "save success: direct write");
            Ok(())
        }
        Err(primary_err) => {
            append_psd_log(
                psd_logs,
                verbose_logging,
                &format!("direct save failed: {}", primary_err),
            );

            if cfg!(windows) {
                let temp_path = std::env::temp_dir().join("tiled-bg-remover-psd-export.psd");
                let temp_target = temp_path.to_string_lossy().to_string();
                append_psd_log(
                    psd_logs,
                    verbose_logging,
                    &format!("retry via temp file: {}", temp_target),
                );

                match document.save(&temp_target) {
                    Ok(_) => {
                        fs::copy(&temp_path, psd_path).map_err(|e| {
                            format!(
                                "PSD temp save succeeded but copy to destination failed: {}",
                                e
                            )
                        })?;
                        let _ = fs::remove_file(&temp_path);
                        append_psd_log(
                            psd_logs,
                            verbose_logging,
                            "save success: temp-file fallback",
                        );
                        Ok(())
                    }
                    Err(temp_err) => {
                        append_psd_log(
                            psd_logs,
                            verbose_logging,
                            &format!("temp save failed: {}", temp_err),
                        );
                        Err(format!(
                            "PSD save failed. direct='{}', temp='{}'",
                            primary_err, temp_err
                        ))
                    }
                }
            } else {
                Err(format!("PSD save failed: {}", primary_err))
            }
        }
    }
}

#[tauri::command]
async fn merge_img(
    tiles: Vec<TileUpdate>,
    original_w: u32,
    original_h: u32,
    overlap_ratio_x: f64,
    overlap_ratio_y: f64,
    key_color: String,
    remove_bg: bool,
    tolerance: u8,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tile_tuples: Vec<(u32, u32, String)> =
            tiles.into_iter().map(|t| (t.r, t.c, t.path)).collect();
        merge_tiles(
            tile_tuples,
            original_w,
            original_h,
            overlap_ratio_x,
            overlap_ratio_y,
            &key_color,
            remove_bg,
            tolerance,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn crop_img(
    state: tauri::State<AppState>,
    path: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let mut state_temp = state
        .temp_dir
        .lock()
        .map_err(|_| "Failed to lock state".to_string())?;

    if state_temp.is_none() {
        *state_temp = Some(TempDir::new().map_err(|e| e.to_string())?);
    }
    let td_path = state_temp.as_ref().unwrap().path().to_path_buf();

    image_processing::crop_image(&path, x, y, width, height, &td_path)
}

#[tauri::command]
fn save_merged_image(path: String, base64_data: String) -> Result<(), String> {
    let data_str = base64_data.split(',').last().unwrap_or(&base64_data);
    let data = general_purpose::STANDARD
        .decode(data_str)
        .map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_export_bundle(
    output_dir: String,
    merged_base64: String,
    tiles: Vec<ExportTile>,
    source_path: Option<String>,
    input_name: Option<String>,
    remove_bg: bool,
    localized_suffix: Option<String>,
    verbose_logging: bool,
) -> Result<SaveBundleResponse, String> {
    if tiles.is_empty() {
        return Err("No tile metadata available for export.".to_string());
    }

    let image_format = if remove_bg {
        ExportImageFormat::Png
    } else {
        ExportImageFormat::Jpeg
    };

    let export_parent = PathBuf::from(output_dir);
    fs::create_dir_all(&export_parent).map_err(|e| e.to_string())?;

    let folder_name = sanitize_path_component(input_name.as_deref().unwrap_or("image"));
    let export_dir = export_parent.join(&folder_name);
    let tiles_dir = export_dir.join("tiles");
    fs::create_dir_all(&tiles_dir).map_err(|e| e.to_string())?;

    let suffix = sanitize_path_component(
        localized_suffix
            .as_deref()
            .unwrap_or("BGRemoved")
            .trim(),
    );
    let stem = if suffix.is_empty() {
        folder_name.clone()
    } else {
        format!("{}_{}", folder_name, suffix)
    };

    let merged_path = export_dir.join(format!("{}.{}", stem, image_format.ext()));
    let psd_path = export_dir.join(format!("{}.psd", stem));
    let mut psd_logs: Vec<String> = Vec::new();

    append_psd_log(
        &mut psd_logs,
        verbose_logging,
        &format!(
            "bundle start: export_dir={}, format={}",
            export_dir.to_string_lossy(),
            image_format.name()
        ),
    );

    let merged_raw = decode_data_url(&merged_base64)?;
    let merged_img = image::load_from_memory(&merged_raw)
        .map_err(|e| format!("Failed to decode merged result: {}", e))?
        .to_rgba8();
    write_image_with_format(&merged_path, &merged_img, image_format)?;

    let mut sorted_tiles = tiles;
    sorted_tiles.sort_unstable_by_key(|t| (t.r, t.c));

    let source_image_path = resolve_input_source_path(source_path.as_deref(), &sorted_tiles)
        .ok_or_else(|| "Failed to resolve input source image for PSD export.".to_string())?;
    let source_bytes = fs::read(&source_image_path).map_err(|e| e.to_string())?;
    let mut source_img = image::load_from_memory(&source_bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();

    if source_img.width() != merged_img.width() || source_img.height() != merged_img.height() {
        source_img = DynamicImage::ImageRgba8(source_img)
            .resize_exact(
                merged_img.width(),
                merged_img.height(),
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8();
    }

    let mut layers: Vec<LayerExport> = Vec::with_capacity(sorted_tiles.len());

    for tile in sorted_tiles {
        let tile_source_path = resolve_tile_source(&tile)?;
        let source_bytes = fs::read(&tile_source_path).map_err(|e| e.to_string())?;
        let mut layer_img = image::load_from_memory(&source_bytes)
            .map_err(|e| e.to_string())?
            .to_rgba8();

        if layer_img.width() != tile.width || layer_img.height() != tile.height {
            layer_img = DynamicImage::ImageRgba8(layer_img)
                .resize_exact(tile.width, tile.height, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
        }

        let tile_export_path =
            tiles_dir.join(format!("tile_r{}_c{}.{}", tile.r + 1, tile.c + 1, image_format.ext()));
        write_image_with_format(&tile_export_path, &layer_img, image_format)?;

        layers.push(LayerExport {
            tile,
            image: layer_img,
        });
    }

    write_psd(
        &psd_path,
        &source_img,
        &merged_img,
        &layers,
        &mut psd_logs,
        verbose_logging,
    )?;

    Ok(SaveBundleResponse {
        export_dir: export_dir.to_string_lossy().to_string(),
        merged_path: merged_path.to_string_lossy().to_string(),
        psd_path: psd_path.to_string_lossy().to_string(),
        tiles_dir: tiles_dir.to_string_lossy().to_string(),
        tile_count: layers.len(),
        image_format: image_format.name().to_string(),
        psd_logs,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            temp_dir: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            split_img,
            merge_img,
            crop_img,
            load_image,
            save_image,
            save_image_resized,
            save_merged_image,
            save_export_bundle
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
