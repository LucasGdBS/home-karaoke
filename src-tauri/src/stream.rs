//! Serves song files (`karaoke.mp4`, `cover.jpg`, `vocals.m4a`) over http://127.0.0.1 with Range support.
//! Why: WebKitGTK (Linux) can't play <video> from Tauri's asset:// protocol (MediaError 4).
//! Bound to localhost only, and every URL carries a random per-run token.
use crate::library;
use std::{collections::hash_map::RandomState, hash::BuildHasher, io::SeekFrom};
use tauri::{AppHandle, State};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub struct Server {
    base: String, // http://127.0.0.1:<port>/<token>
}

pub fn start(app: AppHandle) -> std::io::Result<Server> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    // RandomState is seeded from the OS RNG: good enough for a localhost-only token
    let token = format!("{:016x}{:016x}", RandomState::new().hash_one(1u8), RandomState::new().hash_one(2u8));
    let base = format!("http://127.0.0.1:{port}/{token}");
    tauri::async_runtime::spawn(async move {
        let listener = TcpListener::from_std(listener).expect("listener");
        while let Ok((sock, _)) = listener.accept().await {
            let (app, token) = (app.clone(), token.clone());
            tokio::spawn(async move {
                if let Err(e) = handle(&app, sock, &token).await {
                    eprintln!("stream: {e:#}");
                }
            });
        }
    });
    Ok(Server { base })
}

/// Only these files are served: URL is `<base>/<id>/<file>`.
const SERVED: [(&str, &str); 3] =
    [("karaoke.mp4", "video/mp4"), ("cover.jpg", "image/jpeg"), ("vocals.m4a", "audio/mp4")];

#[tauri::command]
pub fn media_base(server: State<Server>) -> String {
    server.base.clone()
}

async fn handle(app: &AppHandle, mut s: TcpStream, token: &str) -> anyhow::Result<()> {
    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut tmp).await?;
        if n == 0 || buf.len() > 16 * 1024 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let req = String::from_utf8_lossy(&buf);
    let mut lines = req.lines();
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let range = lines.find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.eq_ignore_ascii_case("range").then(|| v.trim().to_string())
    });

    let file = path
        .strip_prefix('/')
        .and_then(|p| p.strip_prefix(token))
        .and_then(|p| p.strip_prefix('/'))
        .and_then(|p| p.split_once('/'))
        .and_then(|(id, name)| {
            let (_, mime) = SERVED.iter().find(|(n, _)| *n == name)?;
            Some((library::dir(app, id).ok()?.join(name), *mime))
        })
        .filter(|_| method == "GET" || method == "HEAD");
    let Some((mut f, mime)) = (match file {
        Some((path, mime)) => tokio::fs::File::open(path).await.ok().map(|f| (f, mime)),
        None => None,
    }) else {
        return reply(&mut s, "404 Not Found", "").await;
    };
    let len = f.metadata().await?.len();
    if len == 0 {
        return reply(&mut s, "404 Not Found", "").await;
    }
    let (status, start, end) = match range.as_deref() {
        None => ("200 OK", 0, len - 1),
        Some(r) => match parse_range(r, len) {
            Some((a, b)) => ("206 Partial Content", a, b),
            None => return reply(&mut s, "416 Range Not Satisfiable", &format!("Content-Range: bytes */{len}\r\n")).await,
        },
    };
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nConnection: close\r\n",
        end - start + 1
    );
    if range.is_some() {
        head += &format!("Content-Range: bytes {start}-{end}/{len}\r\n");
    }
    s.write_all(head.as_bytes()).await?;
    s.write_all(b"\r\n").await?;
    if method == "GET" {
        f.seek(SeekFrom::Start(start)).await?;
        tokio::io::copy(&mut f.take(end - start + 1), &mut s).await?;
    }
    Ok(())
}

async fn reply(s: &mut TcpStream, status: &str, extra: &str) -> anyhow::Result<()> {
    let head = format!("HTTP/1.1 {status}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n");
    s.write_all(head.as_bytes()).await?;
    Ok(())
}

/// "bytes=a-b" | "bytes=a-" | "bytes=-n" → inclusive (start, end). Needs len > 0.
fn parse_range(r: &str, len: u64) -> Option<(u64, u64)> {
    let (a, b) = r.strip_prefix("bytes=")?.split(',').next()?.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", n) => (len.saturating_sub(n.parse().ok()?), len - 1),
        (a, "") => (a.parse().ok()?, len - 1),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(len - 1)),
    };
    (start <= end && end < len).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn parses_ranges() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=-0", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
    }
}
