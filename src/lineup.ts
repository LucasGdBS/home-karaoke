// Singers lineup (#5): who sings next.

/** Swaps item `i` with its neighbour `delta` steps away; out of range leaves the list as is. */
export function move<T>(xs: T[], i: number, delta: number): T[] {
  const j = i + delta;
  if (i < 0 || i >= xs.length || j < 0 || j >= xs.length) return xs;
  const out = [...xs];
  [out[i], out[j]] = [out[j], out[i]];
  return out;
}
