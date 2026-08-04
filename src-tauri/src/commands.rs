use crate::models::{
    AppSettings, DownloadInput, ProgressEvent, PythonMessage, TaskDoneEvent, TaskLogEvent, TaskRecord,
    TaskStatus,
};
use crate::state::AppState;
use crate::state::python_needs_legacy_flag;
use chrono::Utc;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{command, AppHandle, Manager, State};
use uuid::Uuid;

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn read_next_line_lossy<R: Read>(
    reader: &mut BufReader<R>,
    buffer: &mut Vec<u8>,
) -> std::io::Result<Option<String>> {
    buffer.clear();
    let size = reader.read_until(b'\n', buffer)?;
    if size == 0 {
        return Ok(None);
    }
    let line = String::from_utf8_lossy(buffer)
        .trim_end_matches(&['\r', '\n'][..])
        .to_string();
    Ok(Some(line))
}

fn normalize_platform(hint: Option<String>, url: &str) -> String {
    let hint = hint.unwrap_or_else(|| "auto".to_string()).trim().to_lowercase();
    if matches!(hint.as_str(), "bilibili" | "douyin" | "youtube") {
        return hint;
    }

    let lower = url.to_lowercase();
    if lower.contains("bilibili.com") || lower.contains("b23.tv") {
        "bilibili".to_string()
    } else if lower.contains("douyin.com") || lower.contains("iesdouyin.com") {
        "douyin".to_string()
    } else if lower.contains("youtube.com") || lower.contains("youtu.be") {
        "youtube".to_string()
    } else {
        "other".to_string()
    }
}

fn to_task_status(value: &str) -> TaskStatus {
    match value {
        "running" => TaskStatus::Running,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "cancelled" => TaskStatus::Cancelled,
        _ => TaskStatus::Queued,
    }
}

fn emit_progress(app: &AppHandle, payload: &ProgressEvent) {
    let _ = app.emit_all("task_progress", payload);
}

fn emit_log(app: &AppHandle, payload: &TaskLogEvent) {
    let _ = app.emit_all("task_log", payload);
}

fn emit_done(app: &AppHandle, payload: &TaskDoneEvent) {
    let _ = app.emit_all("task_done", payload);
}

fn platform_note(platform: &str) -> &'static str {
    match platform {
        "bilibili" => "Bilibili",
        "douyin" => "Douyin",
        "youtube" => "YouTube",
        _ => "Unknown",
    }
}

fn kill_child(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[command]
pub fn list_tasks(state: State<Arc<AppState>>) -> Vec<TaskRecord> {
    let tasks = state.tasks.lock().unwrap();
    let mut list = tasks.values().cloned().collect::<Vec<_>>();
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    list
}

#[command]
pub fn get_settings(state: State<Arc<AppState>>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[command]
pub fn set_settings(state: State<Arc<AppState>>, settings: AppSettings) -> bool {
    {
        let mut settings_lock = state.settings.lock().unwrap();
        *settings_lock = settings.clone();
    }
    state.save_settings(&settings);
    true
}

#[command]
pub fn pick_output_dir(state: State<Arc<AppState>>) -> String {
    let fallback = {
        let settings = state.settings.lock().unwrap();
        settings.output_dir.clone()
    };

    if let Some(directory) = rfd::FileDialog::new().set_directory(fallback).pick_folder() {
        let dir = directory.to_string_lossy().to_string();
        {
            let mut settings = state.settings.lock().unwrap();
            settings.output_dir = dir.clone();
            state.save_settings(&settings);
        }
        dir
    } else {
        String::new()
    }
}

#[command]
pub fn pick_cookie_file(state: State<Arc<AppState>>) -> String {
    let fallback = {
        let settings = state.settings.lock().unwrap();
        settings.cookie_path.clone()
    };

    let mut dialog = rfd::FileDialog::new();
    if let Some(parent) = PathBuf::from(fallback.clone()).parent() {
        dialog = dialog.set_directory(parent);
    }

    if let Some(cookie_path) = dialog.add_filter("Cookie 文件", &["txt"]).pick_file() {
        let file = cookie_path.to_string_lossy().to_string();
        {
            let mut settings = state.settings.lock().unwrap();
            settings.cookie_path = file.clone();
            state.save_settings(&settings);
        }
        return file;
    }

    if PathBuf::from(fallback.clone()).exists() {
        return fallback;
    }
    String::new()
}

#[command]
pub fn reveal_output_dir(app: AppHandle, state: State<Arc<AppState>>) -> bool {
    let dir = state.settings.lock().unwrap().output_dir.clone();
    if dir.is_empty() {
        emit_log(
            &app,
            &TaskLogEvent {
                task_id: "system".to_string(),
                level: "error".to_string(),
                message: "Output dir is empty".to_string(),
            },
        );
        return false;
    }
    match open::that(&dir) {
        Ok(_) => true,
        Err(_) => {
            emit_log(
                &app,
                &TaskLogEvent {
                    task_id: "system".to_string(),
                    level: "error".to_string(),
                    message: "Failed to open output directory".to_string(),
                },
            );
            false
        }
    }
}

#[command]
pub fn open_file(state: State<Arc<AppState>>, app: AppHandle, task_id: String) -> bool {
    let path = {
        let tasks = state.tasks.lock().unwrap();
        tasks.get(&task_id).and_then(|task| task.file_path.clone())
    };

    let file_path = match path {
        Some(p) => p,
        None => return false,
    };
    let target = PathBuf::from(&file_path);

    if !target.exists() {
        emit_log(
            &app,
            &TaskLogEvent {
                task_id,
                level: "warn".to_string(),
                message: "File not exists".to_string(),
            },
        );
        return false;
    }

    match open::that(&target) {
        Ok(_) => true,
        Err(_) => {
            emit_log(
                &app,
                &TaskLogEvent {
                    task_id,
                    level: "error".to_string(),
                    message: "Failed to open file".to_string(),
                },
            );
            false
        }
    }
}

#[command]
pub fn cancel_task(state: State<Arc<AppState>>, app: AppHandle, task_id: String) -> bool {
    let pid = state.running_pids.lock().unwrap().remove(&task_id);
    let Some(pid) = pid else {
        return false;
    };
    kill_child(pid);

    {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = TaskStatus::Cancelled;
            task.updated_at = now();
            task.error = Some("User cancelled".to_string());
        }
        state.save_tasks(&tasks);
    }

    emit_log(
        &app,
        &TaskLogEvent {
            task_id: task_id.clone(),
            level: "warn".to_string(),
            message: "Task cancelled".to_string(),
        },
    );
    true
}

#[command]
pub fn remove_task(state: State<Arc<AppState>>, app: AppHandle, task_id: String) -> bool {
    {
        let pid = state.running_pids.lock().unwrap().remove(&task_id);
        if let Some(pid) = pid {
            kill_child(pid);
        }
    }
    let mut tasks = state.tasks.lock().unwrap();
    let removed = tasks.remove(&task_id).is_some();
    if removed {
        state.save_tasks(&tasks);
        emit_log(
            &app,
            &TaskLogEvent {
                task_id,
                level: "info".to_string(),
                message: "Task removed".to_string(),
            },
        );
    }
    removed
}

#[command]
pub fn clear_finished_tasks(state: State<Arc<AppState>>, app: AppHandle) -> u32 {
    let mut tasks = state.tasks.lock().unwrap();
    let to_remove: Vec<String> = tasks
        .iter()
        .filter(|(_, t)| {
            matches!(
                t.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        })
        .map(|(id, _)| id.clone())
        .collect();
    let count = to_remove.len() as u32;
    for id in &to_remove {
        tasks.remove(id);
    }
    if count > 0 {
        state.save_tasks(&tasks);
        emit_log(
            &app,
            &TaskLogEvent {
                task_id: "system".to_string(),
                level: "info".to_string(),
                message: format!("Cleared {count} finished tasks"),
            },
        );
    }
    count
}

fn spawn_python_task(
    app: AppHandle,
    state: Arc<AppState>,
    task_id: &str,
    payload: &DownloadInput,
    output_dir: &str,
    platform: &str,
) -> Result<(), String> {
    let safe_output_dir = AppState::normalize_output_dir(output_dir);
    let cookie_path = payload
        .cookie_path
        .clone()
        .filter(|v| !v.trim().is_empty())
        .filter(|v| PathBuf::from(v).exists())
        .or_else(|| {
            if let Some(v) = payload.cookie_path.clone() {
                if !v.trim().is_empty() {
                    emit_log(
                        &app,
                        &TaskLogEvent {
                            task_id: task_id.to_string(),
                            level: "warn".to_string(),
                            message: "Cookie file not found, ignore this cookie".to_string(),
                        },
                    );
                }
            }
            None
        });

    if !state.script_path.exists() {
        return Err("Python script not found: src-tauri/python/downloader_service.py".to_string());
    }

    let input = serde_json::json!({
        "task_id": task_id,
        "url": payload.url,
        "platform": platform,
        "quality": payload.quality.clone().unwrap_or_default(),
        "format": payload.format.clone().unwrap_or_else(|| "mp4".to_string()),
        "vcodec": payload.vcodec.clone().unwrap_or_else(|| "auto".to_string()),
        "output_dir": safe_output_dir.to_string_lossy(),
        "cookie_path": cookie_path,
        "overwrite": payload.overwrite,
        "retry": state.settings.lock().unwrap().retry_count,
    })
    .to_string();

    let mut command = Command::new(&state.python_exec);
    if python_needs_legacy_flag(&state.python_exec) {
        command.arg("-3");
    }
    let mut child = command
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .arg("-u")
        .arg(&state.script_path)
        .arg("--task-json")
        .arg(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to launch python: {err}"))?;

    let pid = child.id();
    state
        .running_pids
        .lock()
        .unwrap()
        .insert(task_id.to_string(), pid);

    let state_for_thread = Arc::clone(&state);
    let app_for_thread = app.clone();
    let task_id_for_thread = task_id.to_string();
    let start = std::time::Instant::now();
    let stderr_buffer = Arc::new(Mutex::new(Vec::<String>::new()));
    let stderr_for_buffer = Arc::clone(&stderr_buffer);

    thread::spawn(move || {
        if let Some(stderr) = child.stderr.take() {
            let app_for_stderr = app_for_thread.clone();
            let task_id_for_stderr = task_id_for_thread.clone();
            let buffer_for_thread = Arc::clone(&stderr_for_buffer);
            thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut line_buffer = Vec::new();
                loop {
                    match read_next_line_lossy(&mut reader, &mut line_buffer) {
                        Ok(Some(raw_line)) => {
                            if raw_line.trim().is_empty() {
                                continue;
                            }
                            {
                                let mut buffer = buffer_for_thread.lock().unwrap();
                                buffer.push(raw_line.clone());
                                if buffer.len() > 80 {
                                    let overflow = buffer.len() - 80;
                                    buffer.drain(0..overflow);
                                }
                            }
                            emit_log(
                                &app_for_stderr,
                                &TaskLogEvent {
                                    task_id: task_id_for_stderr.clone(),
                                    level: "error".to_string(),
                                    message: raw_line,
                                },
                            );
                        }
                        Ok(None) => break,
                        Err(err) => {
                            emit_log(
                                &app_for_stderr,
                                &TaskLogEvent {
                                    task_id: task_id_for_stderr.clone(),
                                    level: "error".to_string(),
                                    message: format!("Read python stderr failed: {err}"),
                                },
                            );
                            break;
                        }
                    }
                }
            });
        }

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout);
            let mut line_buffer = Vec::new();
            loop {
                match read_next_line_lossy(&mut reader, &mut line_buffer) {
                    Ok(Some(raw)) => {
                        if raw.trim().is_empty() {
                            continue;
                        }
                        if let Ok(message) = serde_json::from_str::<PythonMessage>(&raw) {
                            match message {
                                PythonMessage::Progress {
                                    task_id,
                                    percent,
                                    speed,
                                    eta,
                                    status,
                                    message,
                                } => {
                                     let event_speed = speed.clone();
                                     {
                                         let mut tasks = state_for_thread.tasks.lock().unwrap();
                                         if let Some(task) = tasks.get_mut(&task_id) {
                                             task.status = to_task_status(&status);
                                             task.progress = percent;
                                             task.speed = event_speed.clone();
                                             task.eta = eta;
                                             task.updated_at = now();
                                         }
                                         state_for_thread.save_tasks(&tasks);
                                     }
                                     emit_progress(
                                         &app_for_thread,
                                         &ProgressEvent {
                                             task_id,
                                            percent,
                                            speed: event_speed,
                                            eta,
                                            status,
                                            message: message.unwrap_or_else(|| "running".to_string()),
                                        },
                                    );
                                }
                                PythonMessage::Log { task_id, level, message } => {
                                    emit_log(&app_for_thread, &TaskLogEvent {
                                        task_id,
                                        level,
                                        message,
                                    });
                                }
                                PythonMessage::Result {
                                    task_id,
                                    success,
                                    file_path,
                                    duration_ms,
                                    error,
                                    raw,
                                } => {
                                    let persisted_error = if success {
                                        None
                                    } else {
                                        raw.clone().or_else(|| error.clone())
                                    };
                                    {
                                        let mut tasks = state_for_thread.tasks.lock().unwrap();
                                        if let Some(task) = tasks.get_mut(&task_id) {
                                            if success {
                                                task.status = TaskStatus::Completed;
                                                task.progress = 100.0;
                                                task.file_path = file_path.clone();
                                                task.error = None;
                                            } else {
                                                task.status = TaskStatus::Failed;
                                                task.error = persisted_error.clone();
                                            }
                                            task.updated_at = now();
                                            state_for_thread.save_tasks(&tasks);
                                        }
                                    }
                                    emit_done(
                                        &app_for_thread,
                                        &TaskDoneEvent {
                                            task_id,
                                            success,
                                            file_path,
                                            duration_ms,
                                            error: error.clone().or(persisted_error),
                                            raw,
                                        },
                                    );
                                }
                            }
                        } else {
                            emit_log(
                                &app_for_thread,
                                &TaskLogEvent {
                                    task_id: task_id_for_thread.clone(),
                                    level: "debug".to_string(),
                                    message: raw,
                                },
                            );
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        emit_log(
                            &app_for_thread,
                            &TaskLogEvent {
                                task_id: task_id_for_thread.clone(),
                                level: "error".to_string(),
                                message: format!("Read python output failed: {err}"),
                            },
                        );
                        break;
                    }
                }
            }
        }

        let exit_code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        let done_payload = {
            let mut tasks = state_for_thread.tasks.lock().unwrap();
            let stderr_tail = {
                let buffer = stderr_buffer.lock().unwrap();
                if buffer.is_empty() {
                    None
                } else {
                    Some(buffer.join("\n"))
                }
            };
            let payload = if let Some(task) = tasks.get_mut(&task_id_for_thread) {
                let terminal = matches!(
                    task.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                );
                let payload = if terminal {
                    None
                } else {
                    let mut raw = format!("process_exit={exit_code}");
                    let task_status = if exit_code == 0 {
                        task.status = TaskStatus::Completed;
                        task.progress = 100.0;
                        task.error = None;
                        TaskStatus::Completed
                    } else {
                        if let Some(stderr_tail) = stderr_tail.clone() {
                            raw = format!("{raw}; stderr={stderr_tail}");
                        }
                        task.status = TaskStatus::Failed;
                        task.error = Some(raw.clone());
                        task.progress = task.progress.min(99.0);
                        TaskStatus::Failed
                    };
                    task.updated_at = now();
                    Some(TaskDoneEvent {
                        task_id: task_id_for_thread.clone(),
                        success: exit_code == 0 && task_status == TaskStatus::Completed,
                        file_path: task.file_path.clone(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: task.error.clone(),
                        raw: Some(raw),
                    })
                };
                payload
            } else {
                None
            };
            state_for_thread.save_tasks(&tasks);
            payload
        };
        if let Some(payload) = done_payload {
            emit_done(&app_for_thread, &payload);
        }
        state_for_thread
            .running_pids
            .lock()
            .unwrap()
            .remove(&task_id_for_thread);
    });

    Ok(())
}

#[command]
pub fn create_task(
    state: State<Arc<AppState>>,
    app: AppHandle,
    input: DownloadInput,
) -> Result<String, String> {
    let raw_url = input.url.trim().to_string();
    let url = input
        .url
        .split_whitespace()
        .collect::<String>()
        .trim_matches(&['"', '\''][..])
        .to_string();
    if url.is_empty() {
        return Err("Invalid URL".to_string());
    }

    if url != raw_url {
        emit_log(
            &app,
            &TaskLogEvent {
                task_id: "system".to_string(),
                level: "warn".to_string(),
                message: format!("Normalize URL from [{raw_url}] to [{url}]"),
            },
        );
    }

    let platform = normalize_platform(input.platform_hint.clone(), &url);
    if platform == "other" {
        return Err("Only bilibili/douyin/youtube are supported".to_string());
    }
    if !url.to_lowercase().starts_with("http") {
        return Err("Invalid URL".to_string());
    }

    let output_dir = {
        let settings = state.settings.lock().unwrap();
        match input.output_dir.clone() {
            Some(v) if !v.trim().is_empty() => v,
            _ => settings.output_dir.clone(),
        }
    };
    let output_dir = AppState::normalize_output_dir(&output_dir).to_string_lossy().to_string();
    let cookie_path = {
        let settings = state.settings.lock().unwrap();
        match input.cookie_path.clone() {
            Some(v) if !v.trim().is_empty() => v,
            _ => settings.cookie_path.clone(),
        }
    };
    let default_vcodec = state.settings.lock().unwrap().vcodec.clone();
    let vcodec = match input.vcodec.clone() {
        Some(v) if !v.trim().is_empty() => v,
        _ => {
            if default_vcodec.trim().is_empty() {
                "auto".to_string()
            } else {
                default_vcodec
            }
        }
    };

    let task_id = Uuid::new_v4().to_string();
    let retry_count = state.settings.lock().unwrap().retry_count;
    let mut record = TaskRecord::new(
        task_id.clone(),
        url.clone(),
        platform.clone(),
        output_dir.clone(),
        retry_count,
    );
    record.quality = input.quality.clone().unwrap_or_default();
    record.format = input.format.clone().unwrap_or_else(|| "mp4".to_string());
    record.vcodec = vcodec.clone();

    {
        let mut tasks = state.tasks.lock().unwrap();
        tasks.insert(task_id.clone(), record);
        state.save_tasks(&tasks);
    }

    emit_log(
        &app,
        &TaskLogEvent {
            task_id: task_id.clone(),
            level: "info".to_string(),
            message: format!("Task queued: {task_id} [{}]", platform_note(&platform)),
        },
    );

    let payload = DownloadInput {
        url,
        platform_hint: Some(platform.clone()),
        output_dir: Some(output_dir.clone()),
        cookie_path: Some(cookie_path),
        quality: input.quality,
        format: input.format,
        vcodec: Some(vcodec),
        overwrite: input.overwrite,
    };

    let state_ref = state.inner().clone();
    if let Err(err) = spawn_python_task(app, state_ref, &task_id, &payload, &output_dir, &platform) {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = TaskStatus::Failed;
            task.error = Some(err.clone());
            task.updated_at = now();
        }
        state.save_tasks(&tasks);
        return Err(err);
    }

    Ok(task_id)
}
