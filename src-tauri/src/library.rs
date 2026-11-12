//! The library is just `appData/songs/<youtube_id>/` folders; `meta.json` holds the state.
use crate::{pipeline::Queue, tools};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Meta {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub track: Option<String>,
    pub duration: f64,
    /// queued → downloading → downloaded → separating → ready | error
    pub status: String,
    pub error: Option<String>,
    pub lyrics_offset_ms: i64,
    pub has_lyrics: bool,
    pub synced: bool,
    /// `vocals.m4a` exists (songs processed before the guide voice don't have it)
    pub has_vocals: bool,
    /// The automatic lyrics sync already ran (it never touches a manual offset)
    pub auto_synced: bool,
}

pub fn songs_dir(app: &AppHandle) -> PathBuf {
    tools::data_dir(app).join("songs")
}

/// The id becomes a folder name, so only real YouTube ids pass (no path traversal).
pub fn dir(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let ok = id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        return Err(format!("id inválido: {id}"));
    }
    Ok(songs_dir(app).join(id))
}

pub fn load(app: &AppHandle, id: &str) -> Option<Meta> {
    let bytes = fs::read(dir(app, id).ok()?.join("meta.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(app: &AppHandle, m: &Meta) -> anyhow::Result<()> {
    let path = dir(app, &m.id).map_err(anyhow::Error::msg)?.join("meta.json");
    fs::write(path, serde_json::to_vec_pretty(m)?)?;
    let _ = app.emit("song-update", m);
    Ok(())
}

/// Load → change → save. `None` if the song was deleted meanwhile.
pub fn update(app: &AppHandle, id: &str, f: impl FnOnce(&mut Meta)) -> Option<Meta> {
    let mut m = load(app, id)?;
    f(&mut m);
    save(app, &m).ok()?;
    Some(m)
}

pub fn list_all(app: &AppHandle) -> Vec<Meta> {
    let Ok(entries) = fs::read_dir(songs_dir(app)) else { return vec![] };
    entries
        .flatten()
        .filter_map(|e| load(app, e.file_name().to_str()?))
        .collect()
}

#[tauri::command]
pub fn list(app: AppHandle) -> Vec<Meta> {
    list_all(&app)
}

/// Adds a song to the queue and returns right away. Re-adding a song that errored retries it.
#[tauri::command]
pub fn enqueue(app: AppHandle, queue: State<Queue>, id: String, title: String, duration: f64) -> Result<Meta, String> {
    if let Some(m) = load(&app, &id).filter(|m| m.status != "error") {
        return Ok(m);
    }
    fs::create_dir_all(dir(&app, &id)?).map_err(|e| e.to_string())?;
    let m = Meta { id: id.clone(), title, duration, status: "queued".into(), ..Default::default() };
    save(&app, &m).map_err(|e| e.to_string())?;
    queue.download(id);
    Ok(m)
}

#[tauri::command]
pub fn delete(app: AppHandle, id: String) -> Result<(), String> {
    fs::remove_dir_all(dir(&app, &id)?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_offset(app: AppHandle, id: String, ms: i64) -> Result<(), String> {
    update(&app, &id, |m| m.lyrics_offset_ms = ms).map(|_| ()).ok_or("música não encontrada".into())
}

#[tauri::command]
pub fn lyrics(app: AppHandle, id: String) -> Result<Option<String>, String> {
    Ok(fs::read_to_string(dir(&app, &id)?.join("lyrics.lrc")).ok())
}
