#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod models;
mod state;

use std::sync::Arc;
use tauri::Builder;
use state::AppState;

fn main() {
    let state = Arc::new(AppState::new());
    let state_for_setup = Arc::clone(&state);

    Builder::default()
        .manage(state)
        .setup(move |_app| {
            let python_exec = state_for_setup.python_exec.clone();
            std::thread::spawn(move || {
                let mut cmd = std::process::Command::new(&python_exec);
                if state::python_needs_legacy_flag(&python_exec) {
                    cmd.arg("-3");
                }
                cmd.args(["-m", "pip", "install", "-U", "yt-dlp"]);

                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                }

                let _ = cmd.status();
            });
            Ok(())
        })
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
