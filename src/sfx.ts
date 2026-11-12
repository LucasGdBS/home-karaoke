// Score reveal sounds, synthesized with Web Audio (no audio files to ship).
let ctx: AudioContext | null = null;
const audio = () => (ctx ??= new AudioContext());

function noiseBuffer(c: AudioContext) {
  const b = c.createBuffer(1, c.sampleRate, c.sampleRate);
  const d = b.getChannelData(0);
  for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  return b;
}

/** Snare roll that builds up for `seconds`. */
export function drumroll(seconds: number) {
  const c = audio();
  const buf = noiseBuffer(c);
  const t0 = c.currentTime + 0.05;
  for (let t = 0; t < seconds; t += 0.045) {
    const src = c.createBufferSource();
    src.buffer = buf;
    const band = c.createBiquadFilter();
    band.type = "bandpass";
    band.frequency.value = 1200;
    const g = c.createGain();
    const at = t0 + t;
    g.gain.setValueAtTime(0.1 + 0.3 * (t / seconds), at);
    g.gain.exponentialRampToValueAtTime(0.001, at + 0.04);
    src.connect(band).connect(g).connect(c.destination);
    src.start(at, Math.random() * 0.9, 0.05);
  }
}

function tone(c: AudioContext, freq: number, at: number, length: number, type: OscillatorType = "triangle") {
  const o = c.createOscillator();
  o.type = type;
  o.frequency.value = freq;
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, at);
  g.gain.exponentialRampToValueAtTime(0.25, at + 0.02);
  g.gain.exponentialRampToValueAtTime(0.0001, at + length);
  o.connect(g).connect(c.destination);
  o.start(at);
  o.stop(at + length);
}

/** Cymbal + major arpeggio for a good score, a sad "womp womp" for a flop. */
export function reveal(good: boolean) {
  const c = audio();
  const t = c.currentTime + 0.02;
  if (good) {
    const src = c.createBufferSource();
    src.buffer = noiseBuffer(c);
    const high = c.createBiquadFilter();
    high.type = "highpass";
    high.frequency.value = 6000;
    const g = c.createGain();
    g.gain.setValueAtTime(0.35, t);
    g.gain.exponentialRampToValueAtTime(0.001, t + 1.2);
    src.connect(high).connect(g).connect(c.destination);
    src.start(t, 0, 1.2);
    [523.25, 659.25, 783.99, 1046.5].forEach((f, i) => tone(c, f, t + i * 0.09, 1.1));
  } else {
    tone(c, 196, t, 0.45, "sawtooth");
    tone(c, 185, t + 0.5, 0.45, "sawtooth");
    tone(c, 174.6, t + 1, 1.1, "sawtooth");
  }
}
