use base64::{engine::general_purpose, Engine as _};
use image::RgbaImage;
use psd_rs::{Document, Layer};
use std::fs;
use std::io::Cursor;
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
    let data_str = base64_data.split(",").last().unwrap_or(&base64_data);
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
    let data_str = base64_data.split(",").last().unwrap_or(&base64_data);
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
    overlap_ratio: f64,
) -> Result<SplitResponse, String> {
    let path_clone = path.clone();

    // Offload heavy image processing to a blocking thread
    let result = tauri::async_runtime::spawn_blocking(move || {
        let td = TempDir::new().map_err(|e| e.to_string())?;
        let td_path = td.path().to_path_buf();

        let (tiles, w, h, new_path) =
            split_image(&path_clone, rows, cols, overlap_ratio, &td_path)?;

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
    merged_path: String,
    psd_path: String,
    tile_count: usize,
}

struct LayerExport {
    tile: ExportTile,
    image: RgbaImage,
}

fn decode_data_url(data: &str) -> Result<Vec<u8>, String> {
    let payload = data.split(',').last().unwrap_or(data);
    general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| e.to_string())
}

fn ensure_png_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if data.starts_with(PNG_SIGNATURE) {
        return Ok(data.to_vec());
    }

    let img = image::load_from_memory(data).map_err(|e| e.to_string())?;
    let mut cursor = Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageOutputFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}

fn resolve_tile_source(tile: &ExportTile) -> Result<(PathBuf, bool), String> {
    let processed = tile.path.trim();
    if !processed.is_empty() {
        let processed_path = PathBuf::from(processed);
        if processed_path.is_file() {
            return Ok((processed_path, false));
        }
    }

    let original = tile.original_path.trim();
    if !original.is_empty() {
        let original_path = PathBuf::from(original);
        if original_path.is_file() {
            return Ok((original_path, true));
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
            let candidate = parent.join("original_source.png");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn write_psd(
    psd_path: &Path,
    source_image: &RgbaImage,
    merged_image: &RgbaImage,
    layers: &[LayerExport],
) -> Result<(), String> {
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

    // Keep a full-size base layer so PSD canvas size always matches merged output dimensions.
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

    document
        .save(psd_path.to_string_lossy().as_ref())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn merge_img(
    tiles: Vec<TileUpdate>,
    original_w: u32,
    original_h: u32,
    overlap_ratio: f64,
    key_color: String,
    remove_bg: bool,
    tolerance: u8,
) -> Result<String, String> {
    // Offload merging to blocking thread
    tauri::async_runtime::spawn_blocking(move || {
        let tile_tuples: Vec<(u32, u32, String)> =
            tiles.into_iter().map(|t| (t.r, t.c, t.path)).collect();
        merge_tiles(
            tile_tuples,
            original_w,
            original_h,
            overlap_ratio,
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

    // Ensure temp dir exists
    if state_temp.is_none() {
        *state_temp = Some(TempDir::new().map_err(|e| e.to_string())?);
    }
    let td_path = state_temp.as_ref().unwrap().path().to_path_buf();

    image_processing::crop_image(&path, x, y, width, height, &td_path)
}

#[tauri::command]
fn save_merged_image(path: String, base64_data: String) -> Result<(), String> {
    let data_str = base64_data.split(",").last().unwrap_or(&base64_data);
    let data = general_purpose::STANDARD
        .decode(data_str)
        .map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_export_bundle(
    path: String,
    merged_base64: String,
    tiles: Vec<ExportTile>,
    source_path: Option<String>,
) -> Result<SaveBundleResponse, String> {
    if tiles.is_empty() {
        return Err("No tile metadata available for export.".to_string());
    }

    let output_path = PathBuf::from(&path);
    let parent = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;

    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("upscaled_image");
    let psd_path = parent.join(format!("{}.psd", stem));

    let merged_raw = decode_data_url(&merged_base64)?;
    let merged_png = ensure_png_bytes(&merged_raw)?;
    fs::write(&output_path, &merged_png).map_err(|e| e.to_string())?;

    let merged_img = image::load_from_memory(&merged_png)
        .map_err(|e| e.to_string())?
        .to_rgba8();

    let mut sorted_tiles = tiles;
    sorted_tiles.sort_unstable_by_key(|t| (t.r, t.c));

    let source_image_path = resolve_input_source_path(source_path.as_deref(), &sorted_tiles)
        .ok_or_else(|| "Failed to resolve input source image for PSD export.".to_string())?;
    let source_bytes = fs::read(&source_image_path).map_err(|e| e.to_string())?;
    let source_png = ensure_png_bytes(&source_bytes)?;
    let mut source_img = image::load_from_memory(&source_png)
        .map_err(|e| e.to_string())?
        .to_rgba8();

    if source_img.width() != merged_img.width() || source_img.height() != merged_img.height() {
        source_img = image::DynamicImage::ImageRgba8(source_img)
            .resize_exact(
                merged_img.width(),
                merged_img.height(),
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8();
    }

    let mut layers: Vec<LayerExport> = Vec::with_capacity(sorted_tiles.len());

    for tile in sorted_tiles {
        let (source_path, _) = resolve_tile_source(&tile)?;
        let source_bytes = fs::read(&source_path).map_err(|e| e.to_string())?;
        let source_png = ensure_png_bytes(&source_bytes)?;
        let mut layer_img = image::load_from_memory(&source_png)
            .map_err(|e| e.to_string())?
            .to_rgba8();

        if layer_img.width() != tile.width || layer_img.height() != tile.height {
            layer_img = image::DynamicImage::ImageRgba8(layer_img)
                .resize_exact(tile.width, tile.height, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
        }

        layers.push(LayerExport {
            tile,
            image: layer_img,
        });
    }

    write_psd(&psd_path, &source_img, &merged_img, &layers)?;

    Ok(SaveBundleResponse {
        merged_path: output_path.to_string_lossy().to_string(),
        psd_path: psd_path.to_string_lossy().to_string(),
        tile_count: layers.len(),
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
