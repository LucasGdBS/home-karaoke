# Karaoke Home

Tauri 2 app (Rust backend + vanilla TS/Vite frontend). Offline karaoke: yt-dlp download → LRCLIB lyrics → MDX-Net (ONNX) vocal removal. UI text is pt-BR.

## Commands
```bash
npm install && bash scripts/fetch-ffmpeg.sh   # once; ffmpeg sidecar → src-tauri/binaries/ (gitignored)
npm run tauri dev                             # Rust changes need a restart; frontend hot-reloads
npm test                                      # node --test tests/*.test.ts (Node type-stripping, imports use .ts)
cargo test --manifest-path src-tauri/Cargo.toml
npx tsc                                       # frontend typecheck
```

## Architecture
- `pipeline.rs`: two queues. The download worker (yt-dlp + lyrics) feeds the separation worker (ONNX + ffmpeg mux). Status lives in `songs/<id>/meta.json` (`queued → downloading → downloaded → separating → ready | error`); in-flight songs resume on launch; `backfill` retries missing covers/lyrics on launch.
- `stream.rs`: localhost HTTP server (random port + token, Range support) serving a whitelist (`SERVED`) of song files. The frontend builds URLs from `media_base`.
- `tools.rs`: yt-dlp, deno and the model are downloaded to app data on first run (yt-dlp must stay updatable); ffmpeg is a Tauri sidecar resolved next to the exe.
- `sync.rs`: automatic lyrics offset, correlating the LRC's "singing now" pattern with `vocals.m4a` loudness (±15s). Runs once per song (`auto_synced`), only while `lyrics_offset_ms == 0`, so manual `[`/`]` adjustments always win.
- `separate.rs`: MDX constants must match the model's UVR config. Vocals = mix − instrumental (no second model pass).
- Frontend: `main.ts` holds all UI; pure logic lives in small tested modules (`lrc.ts`, `score.ts`, `guide.ts`).

## Gotchas
- WebKitGTK (Linux) can't play `<video>` from Tauri's `asset://`, which is why `stream.rs` exists. Don't switch back to `convertFileSrc`.
- Never change `playbackRate` on media in WebKitGTK: it's a GStreamer seek and stutters. `currentTime` is also jittery there (use medians).
- An `el.onclick = () => cond && x()` handler returning `false` cancels the default action (it once broke form submit). Use block bodies.
- New `Meta` fields rely on `#[serde(default)]` so old `meta.json` files still load. Song ids are validated in `library::dir` (path traversal guard): route every id through it.
- Linux build: a Homebrew `pkg-config` on PATH shadows the system one and hides GTK/WebKit. Use `/usr/bin/pkg-config`.
- The bundled ffmpeg is the LGPL build: no libx264 encoder (fine for the app, which only copies video).
- Testing UI in a plain browser: mock `window.__TAURI_INTERNALS__` (`invoke`, `transformCallback`, `metadata`) via Playwright `addInitScript`.

## Workflow
- Feature branches off `develop`, PR into `develop`; `develop` → `main` is a release. Commit messages in Portuguese.
- Never add `Co-Authored-By` lines to commits, and never add "Generated with Claude Code" (or any Claude attribution/session link) to commits or PR descriptions.
- Releases are automatic (`.github/workflows/instaladores.yml`): a merge into `main` touching the app tags highest `vX.Y.Z` + 1 patch and publishes the installers; pushing a `vX.Y.Z` tag releases that exact version. Installer version comes from the tag, not `tauri.conf.json`. PRs into `develop` touching the app run tests + build (~40 min, Windows minutes count double).
