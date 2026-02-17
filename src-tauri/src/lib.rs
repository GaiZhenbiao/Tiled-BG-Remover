use std::sync::Mutex;
use tempfile::TempDir;
use std::fs;
use base64::{Engine as _, engine::general_purpose};

mod image_processing;
use image_processing::{TileInfo, split_image, merge_tiles};

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
    let ext = std::path::Path::new(&path).extension().and_then(|s| s.to_str()).unwrap_or("png");
    let mime = match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "application/octet-stream"
    };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
fn save_image(path: String, base64_data: String) -> Result<(), String> {
    let data_str = base64_data.split(",").last().unwrap_or(&base64_data);
    let data = general_purpose::STANDARD.decode(data_str).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_image_resized(path: String, base64_data: String, width: u32, height: u32) -> Result<(), String> {
    let data_str = base64_data.split(",").last().unwrap_or(&base64_data);
    let data = general_purpose::STANDARD.decode(data_str).map_err(|e| e.to_string())?;
    image_processing::save_resized_tile(&path, &data, width, height)
}

#[tauri::command]
async fn split_img(
    state: tauri::State<'_, AppState>,
    path: String,
    rows: u32,
    cols: u32,
    overlap_ratio: f64
) -> Result<SplitResponse, String> {
    let path_clone = path.clone();
    
    // Offload heavy image processing to a blocking thread
    let result = tauri::async_runtime::spawn_blocking(move || {
        let td = TempDir::new().map_err(|e| e.to_string())?;
        let td_path = td.path().to_path_buf();
        
        let (tiles, w, h, new_path) = split_image(&path_clone, rows, cols, overlap_ratio, &td_path)?;
        
        Ok::<_, String>((td, tiles, w, h, td_path, new_path))
    }).await.map_err(|e| e.to_string())??;
    
    let (td, tiles, w, h, td_path_buf, new_path) = result;
    
    let mut state_temp = state.temp_dir.lock().map_err(|_| "Failed to lock state".to_string())?;
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
        let tile_tuples: Vec<(u32, u32, String)> = tiles.into_iter().map(|t| (t.r, t.c, t.path)).collect();
        merge_tiles(tile_tuples, original_w, original_h, overlap_ratio, &key_color, remove_bg, tolerance)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn crop_img(
    state: tauri::State<AppState>,
    path: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32
) -> Result<String, String> {
    let mut state_temp = state.temp_dir.lock().map_err(|_| "Failed to lock state".to_string())?;
    
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
    let data = general_purpose::STANDARD.decode(data_str).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState { temp_dir: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![split_img, merge_img, crop_img, load_image, save_image, save_image_resized, save_merged_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}