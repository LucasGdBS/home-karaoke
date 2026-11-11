import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { parseLrc, lineAt, type Line } from "./lrc.ts";

type Meta = {
  id: string;
  title: string;
  artist: string | null;
  duration: number;
  status: string;
  error: string | null;
  lyrics_offset_ms: number;
  has_lyrics: boolean;
  synced: boolean;
};
type SearchResult = { id: string; title: string; duration: number; channel: string };

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const show = (page: string) => ["setup", "home", "player"].forEach((p) => ($(p).hidden = p !== page));
const fmt = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
const errText = (e: unknown) => (typeof e === "string" ? e : String(e));

function el<K extends keyof HTMLElementTagNameMap>(tag: K, text = "", cls = ""): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  e.textContent = text; // never innerHTML: titles come from YouTube
  if (cls) e.className = cls;
  return e;
}
function button(text: string, onclick: () => void, cls = "") {
  const b = el("button", text, cls);
  b.onclick = onclick;
  return b;
}

// ---------- setup ----------
async function runSetup() {
  show("setup");
  for (;;) {
    try {
      $("setup-error").textContent = "";
      $("setup-retry").hidden = true;
      await invoke("setup_tools");
      return;
    } catch (e) {
      $("setup-error").textContent = errText(e);
      $("setup-retry").hidden = false;
      await new Promise((r) => ($("setup-retry").onclick = r));
    }
  }
}

// ---------- library ----------
const songs = new Map<string, Meta>();
const progress = new Map<string, number>();
const STATUS: Record<string, string> = {
  queued: "na fila",
  downloading: "baixando",
  downloaded: "esperando a IA",
  separating: "tirando a voz",
  ready: "pronta",
  error: "erro",
};

function renderLibrary() {
  const items = [...songs.values()].sort((a, b) => a.title.localeCompare(b.title));
  $("library-empty").hidden = items.length > 0;
  $("library").replaceChildren(
    ...items.map((m) => {
      const li = el("li");
      const info = el("div", "", "info");
      info.append(el("strong", m.title), el("span", fmt(m.duration), "muted"));
      const p = progress.get(m.id);
      const busy = m.status === "downloading" || m.status === "separating";
      const status = el("span", STATUS[m.status] ?? m.status, `status ${m.status}`);
      if (busy && p !== undefined) status.textContent += ` ${Math.round(p * 100)}%`;
      if (m.status === "ready" && !m.has_lyrics) status.textContent += " · sem letra";
      if (m.error) status.title = m.error;
      const actions = el("div", "", "actions");
      if (m.status === "ready") actions.append(button("▶ Cantar", () => play(m)));
      if (m.status === "error")
        actions.append(button("Tentar de novo", () => enqueue(m.id, m.title, m.duration), "ghost"));
      actions.append(
        button("Apagar", async () => {
          if (!confirm(`Apagar "${m.title}"?`)) return;
          try {
            await invoke("delete", { id: m.id });
            songs.delete(m.id);
            renderLibrary();
          } catch (e) {
            alert(errText(e));
          }
        }, "ghost"),
      );
      li.append(info, status, actions);
      return li;
    }),
  );
}

async function enqueue(id: string, title: string, duration: number) {
  try {
    const m = await invoke<Meta>("enqueue", { id, title, duration });
    songs.set(m.id, m);
    renderLibrary();
  } catch (e) {
    alert(errText(e));
  }
}

// ---------- search ----------
$("search-form").onsubmit = async (ev) => {
  ev.preventDefault();
  const q = $<HTMLInputElement>("q").value.trim();
  if (!q) return;
  $("search-status").textContent = "Buscando…";
  $("results").replaceChildren();
  try {
    const results = await invoke<SearchResult[]>("search", { q });
    $("search-status").textContent = results.length ? "" : "Nada encontrado.";
    $("results").replaceChildren(
      ...results.map((r) => {
        const li = el("li");
        const img = el("img");
        img.src = `https://i.ytimg.com/vi/${r.id}/mqdefault.jpg`;
        img.alt = "";
        const info = el("div", "", "info");
        info.append(el("strong", r.title), el("span", `${r.channel} · ${fmt(r.duration)}`, "muted"));
        const queued = songs.has(r.id) && songs.get(r.id)!.status !== "error";
        const b = button(queued ? "Já está na lista" : "Baixar", async () => {
          b.disabled = true;
          b.textContent = "Na fila ✓";
          await enqueue(r.id, r.title, r.duration);
        });
        b.disabled = queued;
        li.append(img, info, b);
        return li;
      }),
    );
  } catch (e) {
    $("search-status").textContent = `Erro na busca: ${errText(e)}`;
  }
};

$("update-ytdlp").onclick = async () => {
  const b = $<HTMLButtonElement>("update-ytdlp");
  b.disabled = true;
  try {
    alert(await invoke<string>("update_ytdlp"));
  } catch (e) {
    alert(errText(e));
  } finally {
    b.disabled = false;
  }
};

// ---------- player ----------
const video = $<HTMLVideoElement>("video");
let current: Meta | null = null;
let lines: Line[] = [];
let raf = 0;
let hintTimer = 0;

async function play(m: Meta) {
  current = m;
  const text = m.has_lyrics ? await invoke<string | null>("lyrics", { id: m.id }) : null;
  lines = text && m.synced ? parseLrc(text) : [];
  $("plain").textContent = text && !m.synced ? text : "";
  $("plain").hidden = !(text && !m.synced);
  $("line-cur").textContent = text ? "" : "Letra não encontrada";
  $("line-next").textContent = "";
  video.src = await invoke<string>("video_url", { id: m.id });
  show("player");
  await getCurrentWindow().setFullscreen(true);
  tick();
  video.play().catch((e) => showError(`${e}`));
}

function showError(msg: string) {
  const err = video.error;
  const detail = err ? ` (MediaError ${err.code}${err.message ? `: ${err.message}` : ""})` : "";
  const codecs = video.canPlayType('video/mp4; codecs="avc1.4D401F, mp4a.40.2"') || "não";
  $("line-cur").textContent = "Não consegui tocar o vídeo. Esc para voltar.";
  $("line-next").textContent = `${msg}${detail} · suporta H.264/AAC: ${codecs}`;
  lines = []; // keep the error on screen instead of lyrics
}
video.onerror = () => showError("erro ao carregar");

function tick() {
  if (current && lines.length) {
    const i = lineAt(lines, video.currentTime - current.lyrics_offset_ms / 1000);
    $("line-cur").textContent = lines[i]?.text ?? "";
    $("line-next").textContent = lines[i + 1]?.text ?? "";
  }
  raf = requestAnimationFrame(tick);
}

async function stop() {
  cancelAnimationFrame(raf);
  video.pause();
  video.removeAttribute("src");
  video.load();
  current = null;
  await getCurrentWindow().setFullscreen(false);
  show("home");
}

function hint(text: string) {
  $("hint").textContent = text;
  $("hint").classList.add("on");
  clearTimeout(hintTimer);
  hintTimer = window.setTimeout(() => $("hint").classList.remove("on"), 1500);
}

video.onended = stop;
video.onclick = () => (video.paused ? video.play() : video.pause());
document.addEventListener("keydown", (e) => {
  if (!current) return;
  if (e.key === "Escape") stop();
  else if (e.key === " ") {
    e.preventDefault();
    video.paused ? video.play() : video.pause();
  } else if (e.key === "[" || e.key === "]") {
    // positive offset = lyrics show up later
    current.lyrics_offset_ms += e.key === "]" ? 250 : -250;
    songs.set(current.id, current);
    hint(`Letra: ${(current.lyrics_offset_ms / 1000).toFixed(2)}s`);
    invoke("set_offset", { id: current.id, ms: current.lyrics_offset_ms }).catch(() => {});
  }
});

// ---------- start ----------
listen<Meta>("song-update", (e) => {
  songs.set(e.payload.id, e.payload);
  if (e.payload.status !== "downloading" && e.payload.status !== "separating") progress.delete(e.payload.id);
  renderLibrary();
});
listen<{ id: string; progress: number }>("song-progress", (e) => {
  progress.set(e.payload.id, e.payload.progress);
  renderLibrary();
});
listen<{ item: string; progress: number }>("setup-progress", (e) => {
  $("setup-item").textContent = `Baixando ${e.payload.item}…`;
  $<HTMLProgressElement>("setup-bar").value = e.payload.progress;
});

(async () => {
  if (!(await invoke<boolean>("setup_status"))) await runSetup();
  for (const m of await invoke<Meta[]>("list")) songs.set(m.id, m);
  renderLibrary();
  show("home");
})();
