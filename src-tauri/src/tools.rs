//! External tools: yt-dlp + deno + the MDX model are downloaded to app data on first run
//! (yt-dlp breaks often, so it must be updatable); ffmpeg ships as a Tauri sidecar.
use std::{env, fs, path::Path, path::PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

const EXE: &str = env::consts::EXE_SUFFIX;
const MODEL_FILE: &str = "UVR-MDX-NET-Inst_HQ_3.onnx";
const MODEL_URL: &str =
    "https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/UVR-MDX-NET-Inst_HQ_3.onnx";
#[cfg(windows)]
const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
#[cfg(not(windows))]
const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
// shortcut: x86_64 only, add aarch64 URLs when someone needs ARM laptops
#[cfg(windows)]
const DENO_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";
#[cfg(not(windows))]
const DENO_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip";

pub fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("no app data dir")
}
fn bin_dir(app: &AppHandle) -> PathBuf {
    data_dir(app).join("bin")
}
fn ytdlp(app: &AppHandle) -> PathBuf {
    bin_dir(app).join(format!("yt-dlp{EXE}"))
}
fn deno(app: &AppHandle) -> PathBuf {
    bin_dir(app).join(format!("deno{EXE}"))
}
pub fn model(app: &AppHandle) -> PathBuf {
    data_dir(app).join("models").join(MODEL_FILE)
}
/// Sidecars are placed next to the app executable (in dev too, by tauri-build).
pub fn ffmpeg() -> PathBuf {
    let exe = env::current_exe().expect("no current exe");
    exe.parent().unwrap().join(format!("ffmpeg{EXE}"))
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// std Command that doesn't flash a console window on Windows.
pub fn std_cmd(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, CREATE_NO_WINDOW);
    c
}

/// yt-dlp needs a JS runtime (deno) on PATH for YouTube.
pub fn ytdlp_cmd(app: &AppHandle) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(ytdlp(app));
    let path = env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin_dir(app)).chain(env::split_paths(&path));
    c.env("PATH", env::join_paths(paths).expect("bad PATH"));
    c.kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

pub fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("home-karaoke/0.1")
        .build()
        .expect("http client")
}

#[derive(Clone, serde::Serialize)]
struct SetupProgress<'a> {
    item: &'a str,
    progress: f32,
}

/// Downloads to `<dest>.part` and renames, so `dest.exists()` means complete.
async fn download(app: &AppHandle, url: &str, dest: &Path, item: &str) -> anyhow::Result<()> {
    let mut resp = http().get(url).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(0) as f32;
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    let mut file = tokio::fs::File::create(&part).await?;
    let mut done = 0f32;
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        done += chunk.len() as f32;
        if total > 0.0 {
            let _ = app.emit("setup-progress", SetupProgress { item, progress: done / total });
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&part, dest).await?;
    make_executable(dest)?;
    Ok(())
}

fn make_executable(_p: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(_p, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[tauri::command]
pub fn setup_status(app: AppHandle) -> bool {
    ytdlp(&app).exists() && deno(&app).exists() && model(&app).exists()
}

#[tauri::command]
pub async fn setup_tools(app: AppHandle) -> Result<(), String> {
    setup(&app).await.map_err(|e| format!("{e:#}"))
}

async fn setup(app: &AppHandle) -> anyhow::Result<()> {
    fs::create_dir_all(bin_dir(app))?;
    fs::create_dir_all(model(app).parent().unwrap())?;
    if !ytdlp(app).exists() {
        download(app, YTDLP_URL, &ytdlp(app), "yt-dlp").await?;
    }
    if !deno(app).exists() {
        let zip_path = bin_dir(app).join("deno.zip");
        download(app, DENO_URL, &zip_path, "deno").await?;
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip_path)?)?;
        let mut entry = archive.by_name(&format!("deno{EXE}"))?;
        let mut out = fs::File::create(deno(app))?;
        std::io::copy(&mut entry, &mut out)?;
        drop(out);
        make_executable(&deno(app))?;
        fs::remove_file(&zip_path)?;
    }
    if !model(app).exists() {
        download(app, MODEL_URL, &model(app), "modelo de IA").await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let out = ytdlp_cmd(&app).arg("-U").output().await.map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if out.status.success() { Ok(text) } else { Err(text) }
}
