use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

// ─── Data Models ────────────────────────────────────────────────────

/// A user-created note with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

/// System information for display.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub hostname: String,
    pub cpu_cores: u32,
    pub memory_total_mb: u64,
    pub tauri_version: String,
    pub rust_version: String,
}

/// A file or directory entry.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: String,
}

/// Global application state shared across Tauri commands.
pub struct AppState {
    pub notes: Mutex<Vec<Note>>,
    pub notes_dir: Mutex<std::path::PathBuf>,
}

// ─── System Commands ────────────────────────────────────────────────────

/// Returns system information including OS, hostname, CPU cores, and memory.
#[tauri::command]
fn get_system_info() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        hostname: hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".into()),
        cpu_cores: num_cpus::get() as u32,
        memory_total_mb: sys_info::mem_info()
            .map(|m| m.total / 1024)
            .unwrap_or(0),
        tauri_version: tauri::BUILD_TARGET_TRIPLE.to_string(),
        rust_version: rustc_version_runtime::version().to_string(),
    }
}

// ─── Note Commands ────────────────────────────────────────────────────

/// Returns all notes sorted by last-modified time (newest first).
#[tauri::command]
fn load_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let notes = state.notes.lock().map_err(|e| e.to_string())?;
    let mut sorted = notes.clone();
    sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sorted)
}

/// Creates a new note with the given title and content.
///
/// Persists the note to disk and adds it to the in-memory store.
#[tauri::command]
fn create_note(
    title: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let now = now_string();
    let id = uuid::Uuid::new_v4().to_string();

    let note = Note {
        id: id.clone(),
        title: title.clone(),
        content: content.clone(),
        created_at: now.clone(),
        updated_at: now,
    };

    save_note_to_disk(&id, &note, &state.notes_dir)?;

    state
        .notes
        .lock()
        .map_err(|e| e.to_string())?
        .push(note.clone());

    Ok(note)
}

/// Updates an existing note by ID.
///
/// Updates the title, content, and timestamp, then persists to disk.
#[tauri::command]
fn update_note(
    id: String,
    title: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let now = now_string();

    {
        let mut notes = state.notes.lock().map_err(|e| e.to_string())?;
        let note = notes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| "Note not found".to_string())?;

        note.title = title;
        note.content = content;
        note.updated_at = now;
    }

    // Re-read the note for disk persistence (lock already released)
    let note = {
        let notes = state.notes.lock().map_err(|e| e.to_string())?;
        notes
            .iter()
            .find(|n| n.id == id)
            .cloned()
            .ok_or_else(|| "Note not found after update".to_string())?
    };

    save_note_to_disk(&id, &note, &state.notes_dir)?;

    Ok(note)
}

/// Deletes a note by ID, removing it from both memory and disk.
#[tauri::command]
fn delete_note(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let pos = state
        .notes
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .position(|n| n.id == id)
        .ok_or("Note not found")?;

    state.notes.lock().map_err(|e| e.to_string())?.remove(pos);

    // Remove disk file (ignore errors — file may already be gone)
    if let Ok(notes_dir) = state.notes_dir.lock() {
        let file_path = notes_dir.join(format!("{}.json", id));
        if let Err(e) = std::fs::remove_file(&file_path) {
            tracing::warn!("Failed to remove note file {:?}: {}", file_path, e);
        }
    }

    Ok(true)
}

// ─── Filesystem Commands ────────────────────────────────────────────────

/// Lists the contents of a directory, sorted with directories first
/// then alphabetically (case-insensitive).
#[tauri::command]
fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err("Not a directory".to_string());
    }

    let entries: Result<Vec<FileEntry>, String> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| -> Result<FileEntry, String> {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let modified = metadata
                .modified()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_default();

            Ok(FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
                size: metadata.len(),
                is_dir: metadata.is_dir(),
                modified,
            })
        })
        .collect();

    let mut entries = entries?;

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Reads the full contents of a text file.
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {}", e))
}

/// Writes text content to a file, creating or overwriting it.
#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<bool, String> {
    std::fs::write(&path, content).map_err(|e| format!("写入失败: {}", e))?;
    Ok(true)
}

// ─── App Bootstrap ────────────────────────────────────────────────────────

/// Runs the Tauri application with all plugins and commands registered.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let notes_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev-notes");

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

// ─── Helpers ─────────────────────────────────────────────────────────────

/// Returns the current local time as a formatted string.
fn now_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Persists a single note to the notes directory as a JSON file.
fn save_note_to_disk(
    id: &str,
    note: &Note,
    notes_dir_lock: &Mutex<std::path::PathBuf>,
) -> Result<(), String> {
    let notes_dir = notes_dir_lock.lock().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&*notes_dir).map_err(|e| e.to_string())?;

    let file_path = notes_dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(note).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, json).map_err(|e| e.to_string())?;

    Ok(())
}

/// Loads all `.json` note files from the notes directory.
fn load_notes_from_disk(notes_dir: &std::path::Path) -> Vec<Note> {
    if !notes_dir.exists() {
        return Vec::new();
    }

    std::fs::read_dir(notes_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .filter_map(|entry| std::fs::read_to_string(&entry.path()).ok())
        .filter_map(|content| serde_json::from_str::<Note>(&content).ok())
        .collect()
}
