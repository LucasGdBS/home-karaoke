# Karaoke Home

Karaokê offline para festas: você busca uma música, o app baixa do YouTube, busca a letra sincronizada (LRCLIB) e tira a voz com IA (MDX-Net, roda local, sem API key). Depois disso, toca sem internet.

## Como funciona
Busca → **Baixar** (entra na fila, pode escolher outras) → baixa o vídeo (yt-dlp) + letra → tira a voz (IA) → **pronta**.
Enquanto uma música está na IA, a próxima já vai baixando.

No player: `espaço` pausa, `[` / `]` adiantam ou atrasam a letra 0,25s (fica salvo por música), `Esc` volta.

## Dev

Pré-requisitos: Node, Rust (`rustup`) e os [pré-requisitos do Tauri](https://tauri.app/start/prerequisites/).

- **Linux / WSL**:
  `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libssl-dev pkg-config build-essential`
  Para tocar H.264/AAC no WebKitGTK: `sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good gstreamer1.0-plugins-bad`
- **Windows**: MSVC Build Tools + WebView2 (já vem no Win10/11).

```sh
npm install
bash scripts/fetch-ffmpeg.sh      # Windows: pwsh scripts/fetch-ffmpeg.ps1
npm run tauri dev
```

Testes: `npm test` (parser de letra) e `cargo test --manifest-path src-tauri/Cargo.toml` (STFT, limpeza de título, escolha de letra).

Release: dê push numa tag `v*` e o GitHub Actions gera `.msi`/`.exe` (Windows) e `.AppImage`/`.deb` (Linux).

## Onde ficam as coisas
`<app data>/` (Linux `~/.local/share/com.homekaraoke.app`, Windows `%APPDATA%\com.homekaraoke.app`):
- `bin/`: yt-dlp e deno (baixados no 1º uso; o botão "Atualizar yt-dlp" atualiza o yt-dlp)
- `models/`: o modelo de IA (~66MB)
- `songs/<id>/`: `karaoke.mp4`, `lyrics.lrc`, `meta.json`

## Se der problema
- **Busca/download falha do nada**: o YouTube mudou algo. Clique em "Atualizar yt-dlp".
- **Música com erro**: passe o mouse no "erro" para ver o motivo, ou clique em "Tentar de novo".
- **Letra atrasada/adiantada**: `[` / `]` no player.
