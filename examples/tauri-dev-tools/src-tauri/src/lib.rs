use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

// ─── 应用状态 ────────────────────────────────────────────────────

/// 笔记条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 系统信息
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub hostname: String,
    pub cpu_cores: u32,
    pub memory_total_mb: u64,
    pub tauri_version: String,
    pub rust_version: String,
}

/// 文件条目
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: String,
}

/// 应用全局状态
pub struct AppState {
    pub notes: Mutex<Vec<Note>>,
    pub notes_dir: Mutex<std::path::PathBuf>,
}

// ─── 系统命令 ────────────────────────────────────────────────────

/// 获取系统信息
#[tauri::command]
fn get_system_info() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        hostname: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".into()),
        cpu_cores: num_cpus::get() as u32,
        memory_total_mb: sys_info::mem_info()
            .map(|m| m.total / 1024)
            .unwrap_or(0),
        tauri_version: tauri::BUILD_TARGET_TRIPLE.to_string(),
        rust_version: rustc_version_runtime::version().to_string(),
    }
}

// ─── 笔记命令 ────────────────────────────────────────────────────

/// 加载所有笔记
#[tauri::command]
fn load_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let notes = state.notes.lock().map_err(|e| e.to_string())?;
    let mut sorted = notes.clone();
    sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sorted)
}

/// 创建笔记
#[tauri::command]
fn create_note(
    title: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = uuid::Uuid::new_v4().to_string();

    let note = Note {
        id: id.clone(),
        title: title.clone(),
        content: content.clone(),
        created_at: now.clone(),
        updated_at: now,
    };

    // 保存到文件系统
    let notes_dir = state.notes_dir.lock().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&*notes_dir).map_err(|e| e.to_string())?;

    let file_path = notes_dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&note).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, &json).map_err(|e| e.to_string())?;

    // 添加到内存状态
    let mut notes = state.notes.lock().map_err(|e| e.to_string())?;
    notes.push(note.clone());

    Ok(note)
}

/// 更新笔记
#[tauri::command]
fn update_note(
    id: String,
    title: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut notes = state.notes.lock().map_err(|e| e.to_string())?;
    let note = notes
        .iter_mut()
        .find(|n| n.id == id)
        .ok_or_else(|| "Note not found".to_string())?;

    note.title = title;
    note.content = content;
    note.updated_at = now.clone();

    // 同步到文件系统
    let notes_dir = state.notes_dir.lock().map_err(|e| e.to_string())?;
    let file_path = notes_dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&*note).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, &json).map_err(|e| e.to_string())?;

    Ok(note.clone())
}

/// 删除笔记
#[tauri::command]
fn delete_note(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut notes = state.notes.lock().map_err(|e| e.to_string())?;
    let pos = notes.iter().position(|n| n.id == id).ok_or("Note not found")?;
    notes.remove(pos);

    // 删除文件
    let notes_dir = state.notes_dir.lock().map_err(|e| e.to_string())?;
    let file_path = notes_dir.join(format!("{}.json", id));
    let _ = std::fs::remove_file(&file_path);

    Ok(true)
}

// ─── 文件系统命令 ────────────────────────────────────────────────

/// 列出目录内容
#[tauri::command]
fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err("Not a directory".to_string());
    }

    let mut entries: Vec<FileEntry> = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            modified: metadata
                .modified()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_default(),
        });
    }

    // 目录优先，再按名称排序
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// 读取文件内容
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {}", e))?;
    Ok(content)
}

/// 写入文件
#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<bool, String> {
    std::fs::write(&path, &content).map_err(|e| format!("写入失败: {}", e))?;
    Ok(true)
}

// ─── 应用启动 ────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 笔记存储目录（用户数据目录）
    let notes_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev-notes");

    // 启动时加载已有笔记
    let initial_notes = load_notes_from_disk(&notes_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            notes: Mutex::new(initial_notes),
            notes_dir: Mutex::new(notes_dir),
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            load_notes,
            create_note,
            update_note,
            delete_note,
            list_directory,
            read_text_file,
            write_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 从磁盘加载已有笔记
fn load_notes_from_disk(notes_dir: &std::path::Path) -> Vec<Note> {
    if !notes_dir.exists() {
        return Vec::new();
    }

    let mut notes = Vec::new();

    if let Ok(entries) = std::fs::read_dir(notes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(note) = serde_json::from_str::<Note>(&content) {
                        notes.push(note);
                    }
                }
            }
        }
    }

    notes
}
