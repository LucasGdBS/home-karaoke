# Karaoke Home

Karaokê offline para festas: você busca uma música, o app baixa do YouTube, busca a letra sincronizada (LRCLIB) e tira a voz com IA (MDX-Net, roda local, sem API key). Depois disso, toca sem internet.

## Como funciona
**Buscar e baixar música** → escolha um resultado (entra na fila, pode escolher outras) → baixa o vídeo e a capa (yt-dlp) + letra → tira a voz (IA) → aparece na galeria.
Enquanto uma música está na IA, a próxima já vai baixando. A barra no rodapé mostra o progresso; clique nela para ver a fila.

No fim de cada música aparece a sua **nota** (sorteada, com suspense).

### Atalhos do player
| Tecla | Ação |
|---|---|
| `espaço` | pausa / continua |
| `↑` / `↓` | volume da **voz guia** (a voz original), de 10% em 10% |
| `V` | liga / desliga a voz guia |
| `[` / `]` | atrasa / adianta a letra 0,25s (fica salvo por música) |
| `Esc` | volta para a galeria |

## Dev

Pré-requisitos: Node, Rust (`rustup`) e os [pré-requisitos do Tauri](https://tauri.app/start/prerequisites/).

- **Linux / WSL**:
  `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libssl-dev pkg-config build-essential`
  Para tocar H.264/AAC no WebKitGTK: `sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good gstreamer1.0-plugins-bad`
  Se usar Homebrew, garanta que o `pkg-config` do brew não esteja na frente do `/usr/bin/pkg-config` no PATH.
- **Windows**: MSVC Build Tools + WebView2 (já vem no Win10/11).

```sh
npm install
bash scripts/fetch-ffmpeg.sh      # Windows: pwsh scripts/fetch-ffmpeg.ps1
npm run tauri dev
```

Testes:
- `npm test`: parser de letra, sorteio da nota, sincronia da voz guia
- `cargo test --manifest-path src-tauri/Cargo.toml`: STFT, limpeza de título, escolha de letra, cabeçalho `Range`

Release: dê push numa tag `v*` e o GitHub Actions gera `.msi`/`.exe` (Windows) e `.AppImage`/`.deb` (Linux).

## Onde ficam as coisas
`<app data>/` (Linux `~/.local/share/com.homekaraoke.app`, Windows `%APPDATA%\com.homekaraoke.app`):
- `bin/`: yt-dlp e deno (baixados no 1º uso; "Atualizar yt-dlp" fica no rodapé da busca)
- `models/`: o modelo de IA (~66MB)
- `songs/<id>/`: `karaoke.mp4` (vídeo + instrumental), `vocals.m4a` (voz guia), `cover.jpg`, `lyrics.lrc`, `meta.json`

## Se der problema
- **Busca/download falha do nada**: o YouTube mudou algo. Use "Atualizar yt-dlp" no rodapé da busca.
- **Música com erro**: abra a fila (barra do rodapé) para ver o motivo e clicar em "Tentar de novo".
- **Música sem letra ou sem capa**: o app tenta de novo sozinho ao abrir.
- **Sem voz guia**: músicas baixadas antes da voz guia não têm a voz guardada; apague e baixe de novo.
- **Letra atrasada/adiantada**: `[` / `]` no player.
