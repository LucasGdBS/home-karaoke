use crate::tools;
use anyhow::{Context, bail};
use serde::Serialize;
use std::path::Path;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

#[derive(Serialize)]
pub struct SearchResult {
    id: String,
    title: String,
    duration: f64,
    channel: String,
}

#[tauri::command]
pub async fn search(app: AppHandle, q: String) -> Result<Vec<SearchResult>, String> {
    let out = tools::ytdlp_cmd(&app)
        .args(["--flat-playlist", "-J", "--no-warnings"])
        .arg(format!("ytsearch8:{q}"))
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let entries = v["entries"].as_array().cloned().unwrap_or_default();
    Ok(entries
        .iter()
        .filter_map(|e| {
            Some(SearchResult {
                id: e["id"].as_str()?.into(),
                title: e["title"].as_str()?.into(),
                duration: e["duration"].as_f64().unwrap_or(0.0),
                channel: e["channel"].as_str().or(e["uploader"].as_str()).unwrap_or("").into(),
            })
        })
        .collect())
}

/// Downloads `<dir>/video.mp4` (≤720p, prefers H.264 so every webview can play it)
/// and returns yt-dlp's info JSON.
pub async fn download(
    app: &AppHandle,
    id: &str,
    dir: &Path,
    mut on_progress: impl FnMut(f32),
) -> anyhow::Result<serde_json::Value> {
    let mut child = tools::ytdlp_cmd(app)
        .arg("--ffmpeg-location")
        .arg(tools::ffmpeg())
        .args(["-f", "bv*+ba/b", "-S", "res:720,vcodec:h264,acodec:m4a"])
        .args(["--merge-output-format", "mp4", "--remux-video", "mp4"])
        .args(["--write-info-json", "--no-playlist", "--newline", "--no-warnings"])
        .args(["--progress-template", "download:PROG %(progress._percent)s"])
        .arg("-o")
        .arg(dir.join("video.%(ext)s"))
        .arg(format!("https://www.youtube.com/watch?v={id}"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("não consegui rodar o yt-dlp")?;

    let mut stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    while let Some(line) = lines.next_line().await? {
        // shortcut: video and audio are separate downloads, so the bar runs 0→100% twice
        if let Some(p) = line.strip_prefix("PROG ").and_then(|p| p.trim().parse::<f32>().ok()) {
            on_progress(p / 100.0);
        }
    }
    let status = child.wait().await?;
    let err = err_task.await.unwrap_or_default();
    if !status.success() {
        bail!("yt-dlp falhou: {}", err.trim());
    }
    let info = std::fs::read(dir.join("video.info.json")).context("info.json não encontrado")?;
    Ok(serde_json::from_slice(&info)?)
}
