use crate::models::{AppSettings, TaskRecord};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug)]
pub struct AppState {
    pub tasks: Mutex<HashMap<String, TaskRecord>>,
    pub settings: Mutex<AppSettings>,
    pub running_pids: Mutex<HashMap<String, u32>>,
    pub tasks_path: PathBuf,
    pub settings_path: PathBuf,
    pub script_path: PathBuf,
    pub python_exec: String,
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

impl AppState {
    pub fn new() -> Self {
        let base_dir = dirs::config_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("SnapDown");
        let _ = fs::create_dir_all(&base_dir);

        let tasks_path = base_dir.join("tasks.json");
        let settings_path = base_dir.join("settings.json");

        let mut settings = Self::read_settings(&settings_path).unwrap_or_default();
        if settings.output_dir.trim().is_empty() {
            settings.output_dir = AppSettings::default().output_dir;
        }

        let (python_exec, script_path) = Self::resolve_python_and_script();

        Self {
            tasks: Mutex::new(Self::read_tasks(&tasks_path).unwrap_or_default()),
            settings: Mutex::new(settings),
            running_pids: Mutex::new(HashMap::new()),
            tasks_path,
            settings_path,
            script_path,
            python_exec,
        }
    }

    fn resolve_python_and_script() -> (String, PathBuf) {
        if let Some(dir) = exe_dir() {
            let bundle_dir = dir.join("python-bundle");
            let bundled_python = bundle_dir.join("python.exe");
            let bundled_script = bundle_dir.join("downloader_service.py");

            if bundled_python.exists() && bundled_script.exists() {
                return (
                    bundled_python.to_string_lossy().to_string(),
                    bundled_script,
                );
            }
        }

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let script_path = manifest_dir.join("python").join("downloader_service.py");
        let python_exec = Self::resolve_system_python();
        (python_exec, script_path)
    }

    fn resolve_system_python() -> String {
        let candidates = ["python", "python3", "py"];
        for exe in candidates {
            let mut command = std::process::Command::new(exe);
            if exe == "py" {
                command.arg("-3");
            }
            let status = command
                .arg("-c")
                .arg("import yt_dlp")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                return exe.to_string();
            }
        }
        "python".to_string()
    }

    pub fn read_tasks(path: &Path) -> std::io::Result<HashMap<String, TaskRecord>> {
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let text = std::fs::read_to_string(path)?;
        let loaded: HashMap<String, TaskRecord> =
            serde_json::from_str(&text).unwrap_or_default();
        Ok(loaded)
    }

    pub fn save_tasks(&self, tasks: &HashMap<String, TaskRecord>) {
        if let Ok(json) = serde_json::to_string_pretty(tasks) {
            let _ = std::fs::write(&self.tasks_path, json);
        }
    }

    pub fn read_settings(path: &Path) -> std::io::Result<AppSettings> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "settings parse error"))
    }

    pub fn save_settings(&self, settings: &AppSettings) {
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = std::fs::write(&self.settings_path, json);
        }
    }

    pub fn normalize_output_dir(path: &str) -> PathBuf {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return candidate;
        }

        let fallback = dirs::download_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("下载")
            .join("视频");
        let _ = fs::create_dir_all(&fallback);
        fallback
    }
}

pub fn python_needs_legacy_flag(exe: &str) -> bool {
    exe == "py"
}
