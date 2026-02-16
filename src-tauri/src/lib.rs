use std::sync::Mutex;
use tempfile::TempDir;

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
}

#[tauri::command]
fn split_img(
    state: tauri::State<AppState>,
    path: String,
    rows: u32,
    cols: u32,
    overlap_ratio: f64
) -> Result<SplitResponse, String> {
    // Create temp dir if not exists (or new one for each split? Better to reuse or new)
    // Actually, let's create a new one for each split to keep it clean.
    // But we need to keep it alive so files aren't deleted.
    // We store it in state.
    
    let mut state_temp = state.temp_dir.lock().map_err(|_| "Failed to lock state".to_string())?;
    
    // Create new temp dir
    let td = TempDir::new().map_err(|e| e.to_string())?;
    let td_path = td.path().to_path_buf();
    
    // Process
    let (tiles, w, h) = split_image(&path, rows, cols, overlap_ratio, &td_path)?;
    
    // Store temp dir to prevent cleanup
    *state_temp = Some(td);
    
    Ok(SplitResponse {
        tiles,
        original_width: w,
        original_height: h,
        temp_dir: td_path.to_string_lossy().to_string(),
    })
}

#[derive(serde::Deserialize)]
struct TileUpdate {
    r: u32,
    c: u32,
    path: String,
}

#[tauri::command]
fn merge_img(
    tiles: Vec<TileUpdate>,
    original_w: u32,
    original_h: u32,
    overlap_ratio: f64
) -> Result<String, String> {
    let tile_tuples: Vec<(u32, u32, String)> = tiles.into_iter().map(|t| (t.r, t.c, t.path)).collect();
    merge_tiles(tile_tuples, original_w, original_h, overlap_ratio)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { temp_dir: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![split_img, merge_img, crop_img])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}