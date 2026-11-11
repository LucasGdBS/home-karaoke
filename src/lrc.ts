export type Line = { t: number; text: string };

/** Parses .lrc: "[mm:ss.xx]text", possibly several time tags per line. Sorted by time. */
export function parseLrc(src: string): Line[] {
  const out: Line[] = [];
  for (const raw of src.split(/\r?\n/)) {
    const tags = [...raw.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (!tags.length) continue;
    const text = raw.replace(/\[[^\]]*\]/g, "").trim();
    for (const m of tags) out.push({ t: Number(m[1]) * 60 + Number(m[2]), text });
  }
  return out.sort((a, b) => a.t - b.t);
}

/** Index of the line being sung at time t (-1 before the first line). */
export function lineAt(lines: Line[], t: number): number {
  let i = -1;
  while (i + 1 < lines.length && lines[i + 1].t <= t) i++;
  return i;
}
