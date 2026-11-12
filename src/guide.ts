// Keeps the guide voice (<audio>) in step with the video.
// Never touches playbackRate: in WebKitGTK a rate change is a seek inside GStreamer and
// stutters. currentTime is also jittery there, so we look at the median drift and only
// jump the voice, rarely, when it's clearly off. Two local files rarely drift that far.

const RESYNC_OVER = 0.12; // s of median drift before we jump the voice
const MIN_SAMPLES = 20; // ~1/3 s of frames: enough to see past the jitter
const COOLDOWN_MS = 3000; // a jump also glitches a little; never do them back to back

const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
};

/** `drifts` = recent (guide time − video time) samples, in seconds. */
export function shouldResync(drifts: number[], nowMs: number, lastResyncMs: number): boolean {
  if (drifts.length < MIN_SAMPLES || nowMs - lastResyncMs < COOLDOWN_MS) return false;
  return Math.abs(median(drifts)) > RESYNC_OVER;
}
