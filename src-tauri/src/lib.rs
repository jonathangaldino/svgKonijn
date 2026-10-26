mod trace;

use std::sync::{Arc, Mutex};

use image::RgbaImage;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

/// The decoded input image; every trace starts again from this.
#[derive(Default)]
struct Source(Mutex<Option<Arc<RgbaImage>>>);

/// Takes the raw file bytes as the request body (file picker, drop and paste all send bytes).
#[tauri::command]
fn load_image(request: Request<'_>, source: State<'_, Source>) -> Result<(u32, u32), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw image bytes".into());
    };
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let dims = img.dimensions();
    *source.0.lock().unwrap() = Some(Arc::new(img));
    Ok(dims)
}

#[tauri::command(async)]
fn trace(params: trace::Params, source: State<'_, Source>) -> Result<String, String> {
    let img = source.0.lock().unwrap().clone().ok_or("no image loaded")?;
    trace::trace(&img, &params)
}

#[tauri::command(async)]
fn save_svg(app: AppHandle, svg: String, name: String) -> Result<bool, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("SVG", &["svg"])
        .set_file_name(format!("{name}.svg"))
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(path, svg).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
fn copy_svg(app: AppHandle, svg: String) -> Result<(), String> {
    app.clipboard().write_text(svg).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(Source::default())
        .invoke_handler(tauri::generate_handler![load_image, trace, save_svg, copy_svg])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
