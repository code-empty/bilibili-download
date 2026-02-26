use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadInput {
    pub url: String,
    pub platform_hint: Option<String>,
    pub output_dir: Option<String>,
    pub cookie_path: Option<String>,
    pub quality: Option<String>,
    pub format: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub output_dir: String,
    #[serde(default)]
    pub cookie_path: String,
    pub retry_count: u8,
}

impl Default for AppSettings {
    fn default() -> Self {
        let fallback = dirs::download_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("下载")
            .join("视频")
            .to_string_lossy()
            .to_string();

        Self {
            output_dir: fallback,
            cookie_path: String::new(),
            retry_count: 2,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub url: String,
    pub platform: String,
    pub quality: String,
    pub format: String,
    pub status: TaskStatus,
    pub progress: f32,
    pub speed: Option<String>,
    pub eta: Option<u32>,
    pub file_path: Option<String>,
    pub error: Option<String>,
    pub output_dir: String,
    pub created_at: String,
    pub updated_at: String,
    pub retry_count: u8,
}

impl TaskRecord {
    pub fn new(id: String, url: String, platform: String, output_dir: String, retry_count: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            url,
            platform,
            quality: String::new(),
            format: String::new(),
            status: TaskStatus::Queued,
            progress: 0.0,
            speed: None,
            eta: None,
            file_path: None,
            error: None,
            output_dir,
            created_at: now.clone(),
            updated_at: now,
            retry_count,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub task_id: String,
    pub percent: f32,
    pub speed: Option<String>,
    pub eta: Option<u32>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskLogEvent {
    pub task_id: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskDoneEvent {
    pub task_id: String,
    pub success: bool,
    pub file_path: Option<String>,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub raw: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum PythonMessage {
    Progress {
        task_id: String,
        percent: f32,
        speed: Option<String>,
        eta: Option<u32>,
        status: String,
        message: Option<String>,
    },
    Log {
        task_id: String,
        level: String,
        message: String,
    },
    Result {
        task_id: String,
        success: bool,
        file_path: Option<String>,
        duration_ms: u64,
        error: Option<String>,
        raw: Option<String>,
    },
}
