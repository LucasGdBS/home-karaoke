//! Two-stage queue: one worker downloads (yt-dlp + lyrics), another removes vocals.
//! So song B downloads while song A is being separated. One job per stage at a time,
//! because separation already saturates the CPU.
use crate::{library, lyrics, separate, tools, youtube};
use anyhow::{Context, ensure};
use ort::session::Session;
use std::fs;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub struct Queue(UnboundedSender<String>);

impl Queue {
    pub fn download(&self, id: String) {
        let _ = self.0.send(id);
    }
}

#[derive(Clone, serde::Serialize)]
struct Progress<'a> {
    id: &'a str,
    progress: f32,
}

pub fn start(app: AppHandle) -> Queue {
    let (dl_tx, dl_rx) = unbounded_channel();
    let (sep_tx, sep_rx) = unbounded_channel();
    // Resume whatever was in flight when the app closed.
    for m in library::list_all(&app) {
        match m.status.as_str() {
            "queued" | "downloading" => dl_tx.send(m.id).unwrap(),
            "downloaded" | "separating" => sep_tx.send(m.id).unwrap(),
            _ => {}
        }
    }
    tauri::async_runtime::spawn(backfill(app.clone()));
    tauri::async_runtime::spawn(download_worker(app.clone(), dl_rx, sep_tx));
    tauri::async_runtime::spawn(separate_worker(app, sep_rx));
    Queue(dl_tx)
}

/// On launch, ready songs missing a cover or lyrics try again: covers didn't exist in older
/// versions, and lyrics may have failed on a network hiccup or been added to LRCLIB since.
/// Offline: retried next launch.
async fn backfill(app: AppHandle) {
    for m in library::list_all(&app).into_iter().filter(|m| m.status == "ready") {
        let Ok(dir) = library::dir(&app, &m.id) else { continue };
        if !dir.join("cover.jpg").exists() {
            match youtube::download_cover(&app, &m.id, &dir).await {
                Ok(()) => {
                    library::update(&app, &m.id, |_| {}); // emits song-update so the UI shows it
                }
                Err(e) => eprintln!("capa de {}: {e:#}", m.id),
            }
        }
        if !m.has_lyrics {
            if let Err(e) = fetch_lyrics(&app, &m, &dir).await {
                eprintln!("letra de {}: {e:#}", m.id);
            }
        }
    }
}

/// Saves `lyrics.lrc` and flags the song. Not finding any is Ok: the song plays without.
/// Network errors are retried (2s, 4s): a single hiccup used to leave the song without lyrics.
async fn fetch_lyrics(app: &AppHandle, m: &library::Meta, dir: &std::path::Path) -> anyhow::Result<()> {
    let mut wait = 2;
    let found = loop {
        match lyrics::fetch(&m.title, m.artist.as_deref(), m.track.as_deref(), m.duration).await {
            Err(e) if wait <= 4 => {
                eprintln!("letra de {} (tentando de novo em {wait}s): {e:#}", m.id);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                wait *= 2;
            }
            r => break r?,
        }
    };
    if let Some((text, synced)) = found {
        fs::write(dir.join("lyrics.lrc"), text)?;
        library::update(app, &m.id, |m| {
            m.has_lyrics = true;
            m.synced = synced;
        });
    }
    Ok(())
}

/// "Buscar letra" button: tries LRCLIB again now. Returns whether lyrics were found.
#[tauri::command]
pub async fn find_lyrics(app: AppHandle, id: String) -> Result<bool, String> {
    let dir = library::dir(&app, &id)?;
    let m = library::load(&app, &id).ok_or("música não encontrada")?;
    fetch_lyrics(&app, &m, &dir).await.map_err(|e| format!("{e:#}"))?;
    Ok(library::load(&app, &id).is_some_and(|m| m.has_lyrics))
}

fn fail(app: &AppHandle, id: &str, e: anyhow::Error) {
    library::update(app, id, |m| {
        m.status = "error".into();
        m.error = Some(format!("{e:#}"));
    });
}

async fn download_worker(app: AppHandle, mut rx: UnboundedReceiver<String>, sep: UnboundedSender<String>) {
    while let Some(id) = rx.recv().await {
        if library::update(&app, &id, |m| m.status = "downloading".into()).is_none() {
            continue; // deleted while queued
        }
        match download_job(&app, &id).await {
            Ok(()) => {
                if library::update(&app, &id, |m| m.status = "downloaded".into()).is_some() {
                    let _ = sep.send(id);
                }
            }
            Err(e) => fail(&app, &id, e),
        }
    }
}

async fn download_job(app: &AppHandle, id: &str) -> anyhow::Result<()> {
    let dir = library::dir(app, id).map_err(anyhow::Error::msg)?;
    let info = youtube::download(app, id, &dir, |p| {
        let _ = app.emit("song-progress", Progress { id, progress: p });
    })
    .await?;
    let s = |k: &str| info[k].as_str().map(String::from);
    let m = library::update(app, id, |m| {
        m.title = s("title").unwrap_or(m.title.clone());
        m.artist = s("artist").or(s("creator"));
        m.track = s("track");
        m.duration = info["duration"].as_f64().unwrap_or(m.duration);
    })
    .context("música apagada")?;

    // Missing lyrics is not fatal: the song still plays, and `backfill` retries on next launch.
    if let Err(e) = fetch_lyrics(app, &m, &dir).await {
        eprintln!("letra de {id}: {e:#}");
    }
    Ok(())
}

async fn separate_worker(app: AppHandle, mut rx: UnboundedReceiver<String>) {
    let mut session: Option<Session> = None; // loaded once, reused for every song
    while let Some(id) = rx.recv().await {
        if library::update(&app, &id, |m| m.status = "separating".into()).is_none() {
            continue;
        }
        let (app2, id2, mut s) = (app.clone(), id.clone(), session.take());
        let (s, res) = tauri::async_runtime::spawn_blocking(move || {
            let r = separate_job(&app2, &id2, &mut s);
            (s, r)
        })
        .await
        .unwrap_or_else(|e| (None, Err(anyhow::anyhow!("worker panicked: {e}"))));
        session = s;
        match res {
            Ok(()) => {
                library::update(&app, &id, |m| {
                    m.status = "ready".into();
                    m.error = None;
                    m.has_vocals = true;
                });
            }
            Err(e) => fail(&app, &id, e),
        }
    }
}

fn separate_job(app: &AppHandle, id: &str, session: &mut Option<Session>) -> anyhow::Result<()> {
    let dir = library::dir(app, id).map_err(anyhow::Error::msg)?;
    let video = dir.join("video.mp4");
    let raw = dir.join("instrumental.f32");
    let raw_vocals = dir.join("vocals.f32");

    let out = tools::std_cmd(tools::ffmpeg())
        .args(["-v", "error", "-i"])
        .arg(&video)
        .args(["-vn", "-f", "f32le", "-ac", "2", "-ar", "44100", "-"])
        .output()
        .context("não consegui rodar o ffmpeg")?;
    ensure!(out.status.success(), "ffmpeg (decodificar): {}", String::from_utf8_lossy(&out.stderr));
    let mut mix: Vec<f32> = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
    drop(out);

    if session.is_none() {
        *session = Some(Session::builder()?.commit_from_file(tools::model(app)).context("carregar modelo")?);
    }
    let inst = separate::separate(session.as_mut().unwrap(), &mix, |p| {
        let _ = app.emit("song-progress", Progress { id, progress: p });
    })?;
    // The guide voice for free: whatever the model removed. Reuses the mix buffer.
    for (m, i) in mix.iter_mut().zip(&inst) {
        *m -= *i;
    }
    let f32_bytes = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    fs::write(&raw_vocals, f32_bytes(&mix))?;
    drop(mix);
    fs::write(&raw, f32_bytes(&inst))?;
    drop(inst);

    let out = tools::std_cmd(tools::ffmpeg())
        .args(["-v", "error", "-y", "-f", "f32le", "-ar", "44100", "-ac", "2", "-i"])
        .arg(&raw_vocals)
        .args(["-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart"])
        .arg(dir.join("vocals.m4a"))
        .output()
        .context("não consegui rodar o ffmpeg")?;
    ensure!(out.status.success(), "ffmpeg (voz): {}", String::from_utf8_lossy(&out.stderr));

    // One file with the original video + instrumental audio: they can never drift apart.
    let out = tools::std_cmd(tools::ffmpeg())
        .args(["-v", "error", "-y", "-f", "f32le", "-ar", "44100", "-ac", "2", "-i"])
        .arg(&raw)
        .arg("-i")
        .arg(&video)
        .args(["-map", "1:v:0", "-map", "0:a:0", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k"])
        .args(["-shortest", "-movflags", "+faststart"])
        .arg(dir.join("karaoke.mp4"))
        .output()
        .context("não consegui rodar o ffmpeg")?;
    ensure!(out.status.success(), "ffmpeg (juntar): {}", String::from_utf8_lossy(&out.stderr));
    fs::remove_file(&raw)?;
    fs::remove_file(&raw_vocals)?;
    fs::remove_file(&video)?;
    let _ = fs::remove_file(dir.join("video.info.json"));
    Ok(())
}
