mod library;
mod lyrics;
mod pipeline;
mod separate;
mod stream;
mod sync;
mod tools;
mod youtube;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let queue = pipeline::start(app.handle().clone());
            app.manage(queue);
            app.manage(stream::start(app.handle().clone())?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tools::setup_status,
            tools::setup_tools,
            tools::update_ytdlp,
            youtube::search,
            library::list,
            library::enqueue,
            library::delete,
            library::set_offset,
            library::lyrics,
            pipeline::find_lyrics,
            stream::media_base,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
