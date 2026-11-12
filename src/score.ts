// The end-of-song score. Random for now, biased high (like most karaoke machines).
// To score for real (mic + pitch), replace randomScore; the reveal screen stays the same.

export function randomScore(rand: () => number = Math.random): number {
  if (rand() < 0.05) return 30 + Math.floor(rand() * 30); // rare flop, for the laughs: 30–59
  const u = rand();
  return Math.round(60 + 40 * (1 - u * u)); // 60–100, mostly high
}

/** Below this it's a flop (and the sad trombone plays). */
export const FLOP = 69;

export function phrase(score: number): string {
  if (score === 100) return "PERFEITO!";
  if (score >= 90) return "Arrasou!";
  if (score >= FLOP) return "Passou raspando.";
  return "Melhor não largar o emprego…";
}
