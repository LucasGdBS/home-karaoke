import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { parseLrc, lineAt, type Line } from "./lrc.ts";
import { randomScore, phrase, FLOP } from "./score.ts";
import { drumroll, reveal } from "./sfx.ts";
import { shouldResync } from "./guide.ts";

type Meta = {
  id: string;
  title: string;
  artist: string | null;
  track: string | null;
  duration: number;
  status: string;
  error: string | null;
  lyrics_offset_ms: number;
  has_lyrics: boolean;
  synced: boolean;
  has_vocals: boolean;
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
function icon(name: string) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `#i-${name}`);
  svg.append(use);
  return svg;
}
function iconButton(name: string, label: string, onclick: () => void) {
  const b = button("", onclick, "icon");
  b.setAttribute("aria-label", label);
  b.append(icon(name));
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

// ---------- state ----------
let mediaBase = ""; // http://127.0.0.1:<port>/<token>, served by stream.rs
const songs = new Map<string, Meta>();
/** Progress of the current stage, with where it started, to estimate time left. */
const progress = new Map<string, { status: string; p: number; t0: number; p0: number }>();
const BUSY = ["separating", "downloading"];
const STAGE: Record<string, string> = {
  separating: "Tirando a voz",
  downloading: "Baixando",
  downloaded: "Esperando a IA",
  queued: "Na fila",
  error: "Erro",
};
const RANK = ["separating", "downloading", "downloaded", "queued", "error"];

const songTitle = (m: Meta) => m.track ?? m.title;
const initials = (t: string) =>
  t.split(/\s+/).filter((w) => /\p{L}/u.test(w[0] ?? "")).slice(0, 2).map((w) => w[0]).join("").toUpperCase();

function eta(id: string) {
  const g = progress.get(id);
  if (!g || g.p - g.p0 < 0.02) return "calculando…";
  // shortcut: yt-dlp downloads video then audio, so a download's estimate covers the current part only
  const secs = ((Date.now() - g.t0) / 1000 / (g.p - g.p0)) * (1 - g.p);
  return `faltam ~${fmt(secs)}`;
}
const pct = (id: string) => `${Math.round((progress.get(id)?.p ?? 0) * 100)}%`;

// ---------- gallery ----------
function renderGallery() {
  const ready = [...songs.values()]
    .filter((m) => m.status === "ready")
    .sort((a, b) => songTitle(a).localeCompare(songTitle(b)));
  $("ready-count").textContent =
    ready.length === 1 ? "1 música pronta para cantar" : `${ready.length} músicas prontas para cantar`;
  $("library-empty").hidden = ready.length > 0;
  $("gallery").replaceChildren(...ready.map(card));
}

// Success needs no handling here: the backend's song-update re-renders the card with the badge.
function findLyricsButton(m: Meta) {
  const b = button("Sem letra · Buscar", async () => {
    b.disabled = true;
    b.textContent = "Buscando…";
    try {
      if (!(await invoke<boolean>("find_lyrics", { id: m.id }))) b.textContent = "Não achei · Tentar de novo";
    } catch (e) {
      b.textContent = "Erro · Tentar de novo";
      b.title = `${e}`; // the installed Windows app has no console: hovering is the only way to see why
    }
    b.disabled = false;
  }, "badge none find");
  b.title = "Buscar a letra no LRCLIB de novo";
  return b;
}

function card(m: Meta) {
  const title = songTitle(m);
  const c = el("article", "", "card");

  const cover = button("", () => play(m), "cover");
  cover.setAttribute("aria-label", `Cantar ${title}`);
  const img = el("img");
  img.src = `${mediaBase}/${m.id}/cover.jpg`;
  img.alt = "";
  img.onerror = () => img.remove(); // no cover yet: the initials underneath show
  const play_ = el("span", "", "play");
  const circle = el("span");
  circle.append(icon("play"));
  play_.append(circle);
  cover.append(el("span", initials(title), "initials"), img, play_, el("span", fmt(m.duration), "duration"));

  const info = el("div", "", "card-info");
  info.append(el("strong", title), el("span", m.artist ?? "", "muted"));
  if (m.has_lyrics) {
    const [cls, label] = m.synced ? ["synced", "Letra sincronizada"] : ["plain", "Letra sem sincronia"];
    info.append(el("span", label, `badge ${cls}`));
  } else {
    info.append(findLyricsButton(m));
  }

  const body = el("div", "", "card-body");
  body.append(info, iconButton("trash", `Apagar ${title}`, () => remove(m)));
  c.append(cover, body);
  return c;
}

async function remove(m: Meta) {
  if (!confirm(`Apagar "${songTitle(m)}"?`)) return;
  try {
    await invoke("delete", { id: m.id });
    songs.delete(m.id);
    render();
  } catch (e) {
    alert(errText(e));
  }
}

// ---------- status bar + queue ----------
function queueItems() {
  return [...songs.values()]
    .filter((m) => m.status !== "ready")
    .sort((a, b) => RANK.indexOf(a.status) - RANK.indexOf(b.status) || songTitle(a).localeCompare(songTitle(b)));
}

function renderQueue() {
  const items = queueItems();
  const active = items.find((m) => BUSY.includes(m.status));
  const errors = items.filter((m) => m.status === "error").length;
  const waiting = items.filter((m) => m !== active && m.status !== "error").length;

  if (active) {
    $("status-title").textContent = `${STAGE[active.status]} · ${songTitle(active)}`;
    $("status-sub").textContent = `${pct(active.id)} · ${eta(active.id)}`;
    $("status-fill").style.width = pct(active.id);
  } else {
    $("status-title").textContent = "Nenhum download em andamento";
    $("status-sub").textContent = errors ? `${errors} com erro, veja a fila` : "Tudo pronto";
  }
  $("status-track").hidden = !active;
  $("status-count").hidden = waiting === 0;
  $("status-count").textContent = `+${waiting} na fila`;

  $("queue-empty").hidden = items.length > 0;
  $("queue-list").replaceChildren(
    ...items.map((m) => {
      const kind = m === active ? "active" : m.status === "error" ? "error" : BUSY.includes(m.status) ? "" : "waiting";
      const li = el("li", "", kind);
      const row = el("div", "", "q-row");
      row.append(el("span", "", "dot"), el("strong", songTitle(m)), el("span", STAGE[m.status] ?? m.status, "q-stage"));
      li.append(row);
      if (BUSY.includes(m.status)) {
        const detail = el("div", "", "q-detail");
        const track = el("span", "", "track");
        const fill = el("span");
        fill.style.width = pct(m.id);
        track.append(fill);
        detail.append(track, el("span", `${pct(m.id)} · ${eta(m.id)}`, "muted"));
        li.append(detail);
      }
      if (m.status === "error") {
        const detail = el("div", "", "q-detail");
        const msg = el("span", m.error ?? "erro desconhecido", "error");
        msg.title = m.error ?? "";
        detail.append(
          msg,
          button("Tentar de novo", () => enqueue(m.id, m.title, m.duration), "outline"),
          iconButton("trash", `Remover ${songTitle(m)}`, () => remove(m)),
        );
        li.append(detail);
      }
      return li;
    }),
  );
}

function render() {
  renderGallery();
  renderQueue();
}

$("statusbar").onclick = () => {
  const open = $("queue").hidden;
  $("queue").hidden = !open;
  $("statusbar").setAttribute("aria-expanded", String(open));
  $("status-toggle").textContent = open ? "Fechar fila" : "Ver fila";
};

async function enqueue(id: string, title: string, duration: number) {
  try {
    const m = await invoke<Meta>("enqueue", { id, title, duration });
    songs.set(m.id, m);
    render();
  } catch (e) {
    alert(errText(e));
  }
}

// ---------- search ----------
const dialog = $<HTMLDialogElement>("search");
$("open-search").onclick = () => {
  dialog.showModal();
  $<HTMLInputElement>("q").select();
};
$("close-search").onclick = () => dialog.close();
// click on the backdrop closes. Block body: an on* handler returning false cancels the click (and the form submit)
dialog.onclick = (e) => {
  if (e.target === dialog) dialog.close();
};

function resultButton(r: SearchResult) {
  const m = songs.get(r.id);
  const label = !m || m.status === "error" ? "Baixar" : m.status === "ready" ? "Na biblioteca" : "Na fila ✓";
  const b = button(label, async () => {
    b.disabled = true;
    b.textContent = "Na fila ✓";
    b.className = "ghost";
    await enqueue(r.id, r.title, r.duration);
  }, label === "Baixar" ? "primary" : "ghost");
  b.disabled = label !== "Baixar";
  return b;
}

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
        const thumb = el("span", "", "thumb");
        const img = el("img");
        img.src = `https://i.ytimg.com/vi/${r.id}/mqdefault.jpg`;
        img.alt = "";
        thumb.append(img, el("span", fmt(r.duration), "duration"));
        const info = el("span", "", "r-info");
        info.append(el("strong", r.title), el("span", r.channel, "muted"));
        li.append(thumb, info, resultButton(r));
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

// ---------- guide voice ----------
const guide = $<HTMLAudioElement>("guide");
const guideVol = $<HTMLInputElement>("guide-vol");
let hasGuide = false;
let guideCtlTimer = 0;
let drifts: number[] = []; // recent guide−video gaps, for shouldResync()
let lastResync = 0;
const loadPref = (k: string) => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};
const savePref = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private mode etc.: the level just isn't remembered */
  }
};
let guideLevel = 0;
let guideLast = 0.3; // what V brings back after muting

function setGuide(level: number, announce = true) {
  guideLevel = Math.round(Math.min(1, Math.max(0, level)) * 10) / 10;
  if (guideLevel > 0) guideLast = guideLevel;
  guide.volume = guideLevel;
  guideVol.value = String(guideLevel * 100);
  const pct = `${Math.round(guideLevel * 100)}%`;
  $("guide-pct").textContent = pct;
  savePref("guideVolume", String(guideLevel));
  if (announce) hint(`Voz guia: ${pct}`);
}
setGuide(Number(loadPref("guideVolume")) || 0, false);
guideVol.oninput = () => setGuide(Number(guideVol.value) / 100, false);

function showGuideCtl() {
  $("guide-ctl").classList.add("show");
  clearTimeout(guideCtlTimer);
  guideCtlTimer = window.setTimeout(() => $("guide-ctl").classList.remove("show"), 3000);
}
$("player").addEventListener("mousemove", showGuideCtl);

// the voice follows the video; tick() fixes it only when it's clearly off
function alignGuide() {
  guide.currentTime = video.currentTime;
  drifts = [];
  lastResync = performance.now();
}
video.addEventListener("play", () => {
  if (!hasGuide) return;
  alignGuide();
  guide.play().catch(() => {});
});
video.addEventListener("pause", () => guide.pause());
video.addEventListener("seeked", () => hasGuide && alignGuide());

async function play(m: Meta) {
  current = m;
  const text = m.has_lyrics ? await invoke<string | null>("lyrics", { id: m.id }) : null;
  lines = text && m.synced ? parseLrc(text) : [];
  $("plain").textContent = text && !m.synced ? text : "";
  $("plain").hidden = !(text && !m.synced);
  $("line-cur").textContent = text ? "" : "Letra não encontrada";
  $("line-next").textContent = "";
  hasGuide = m.has_vocals;
  $("guide-ctl").hidden = !hasGuide;
  if (hasGuide) guide.src = `${mediaBase}/${m.id}/vocals.m4a`;
  video.src = `${mediaBase}/${m.id}/karaoke.mp4`;
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
  if (hasGuide && !video.paused && !guide.paused) {
    drifts.push(guide.currentTime - video.currentTime);
    if (drifts.length > 30) drifts.shift();
    if (shouldResync(drifts, performance.now(), lastResync)) alignGuide();
  }
  raf = requestAnimationFrame(tick);
}

// ---------- score ----------
let scoreRun = 0; // bumped by stop(), so a reveal still animating gives up
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function showScore() {
  const run = ++scoreRun;
  const final = randomScore();
  const value = $("score-value");
  $("score-phrase").textContent = "";
  $("score-done").hidden = true;
  $("score").hidden = false;
  const seconds = 3.5;
  drumroll(seconds);
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const start = performance.now();
  // numbers spin fast, then slow down like a reel
  for (let delay = 40; performance.now() - start < seconds * 1000; delay *= 1.07) {
    if (run !== scoreRun) return;
    value.textContent = still ? "?" : String(30 + Math.floor(Math.random() * 71));
    await sleep(delay);
  }
  if (run !== scoreRun) return;
  value.textContent = String(final);
  $("score-phrase").textContent = phrase(final);
  reveal(final >= FLOP);
  $("score-done").hidden = false;
  $("score-done").focus();
}
$("score-done").onclick = () => stop();

async function stop() {
  scoreRun++;
  $("score").hidden = true;
  cancelAnimationFrame(raf);
  video.pause();
  video.removeAttribute("src");
  video.load();
  guide.pause();
  guide.removeAttribute("src");
  guide.load();
  hasGuide = false;
  current = null;
  await getCurrentWindow().setFullscreen(false);
  show("home");
}

// Says which way: a bare "-0.25s" left people pressing the key that made it worse (#8).
function offsetLabel(ms: number) {
  const s = (Math.abs(ms) / 1000).toFixed(2).replace(".", ",");
  return ms === 0 ? "Letra no tempo original" : `Letra ${s}s mais ${ms > 0 ? "tarde" : "cedo"}`;
}

function hint(text: string) {
  $("hint").textContent = text;
  $("hint").classList.add("on");
  clearTimeout(hintTimer);
  hintTimer = window.setTimeout(() => $("hint").classList.remove("on"), 1500);
}

video.onended = () => {
  guide.pause();
  showScore();
};
video.onclick = () => (video.paused ? video.play() : video.pause());
document.addEventListener("keydown", (e) => {
  if (!current) return;
  if (e.key === "Escape") stop();
  else if (!$("score").hidden) return; // the score screen only takes Esc (and Enter/Space on its button)
  else if (e.key === " ") {
    e.preventDefault();
    video.paused ? video.play() : video.pause();
  } else if (e.key === "[" || e.key === "]") {
    // positive offset = lyrics show up later
    current.lyrics_offset_ms += e.key === "]" ? 250 : -250;
    songs.set(current.id, current);
    hint(offsetLabel(current.lyrics_offset_ms));
    invoke("set_offset", { id: current.id, ms: current.lyrics_offset_ms }).catch(() => {});
  } else if (e.key === "ArrowUp" || e.key === "ArrowDown" || e.key.toLowerCase() === "v") {
    e.preventDefault(); // also when the slider has focus: ↑/↓ must always step 10% and show the hint
    if (!hasGuide) return hint("Voz guia indisponível: baixe a música de novo");
    if (e.key.toLowerCase() === "v") setGuide(guideLevel > 0 ? 0 : guideLast);
    else setGuide(guideLevel + (e.key === "ArrowUp" ? 0.1 : -0.1));
    showGuideCtl();
  }
});

// ---------- start ----------
listen<Meta>("song-update", (e) => {
  songs.set(e.payload.id, e.payload);
  render();
});
listen<{ id: string; progress: number }>("song-progress", ({ payload: { id, progress: p } }) => {
  const status = songs.get(id)?.status ?? "";
  const g = progress.get(id);
  // new stage, or the bar restarted (yt-dlp's second file): restart the estimate
  if (!g || g.status !== status || p < g.p - 0.05) progress.set(id, { status, p, t0: Date.now(), p0: p });
  else g.p = p;
  renderQueue(); // not the gallery: rebuilding it on every tick would reload the covers
});
listen<{ item: string; progress: number }>("setup-progress", (e) => {
  $("setup-item").textContent = `Baixando ${e.payload.item}…`;
  $<HTMLProgressElement>("setup-bar").value = e.payload.progress;
});

(async () => {
  if (!(await invoke<boolean>("setup_status"))) await runSetup();
  mediaBase = await invoke<string>("media_base");
  for (const m of await invoke<Meta[]>("list")) songs.set(m.id, m);
  render();
  show("home");
})();
