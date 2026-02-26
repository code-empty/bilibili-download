#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod models;
mod state;

use std::sync::Arc;
use tauri::Builder;
use state::AppState;

fn main() {
    let state = Arc::new(AppState::new());

    Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::create_task,
            commands::list_tasks,
            commands::cancel_task,
            commands::open_file,
            commands::reveal_output_dir,
            commands::pick_output_dir,
            commands::pick_cookie_file,
            commands::get_settings,
            commands::set_settings,
            commands::remove_task,
            commands::clear_finished_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
