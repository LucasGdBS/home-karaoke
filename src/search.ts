// Local library search and yt-dlp error classification for the search dialog.

const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** Every word of the query appears in one of the fields, ignoring case and accents. */
export function matches(query: string, fields: (string | null)[]): boolean {
  const hay = fold(fields.filter(Boolean).join(" "));
  const words = fold(query).split(/\s+/).filter(Boolean);
  return words.length > 0 && words.every((w) => hay.includes(w));
}

// Linux: "Failed to resolve ... name resolution"; Windows: "getaddrinfo failed".
const NO_NET = /failed to resolve|name resolution|getaddrinfo|network is unreachable|no route to host/i;

export const isOffline = (err: string) => NO_NET.test(err);
