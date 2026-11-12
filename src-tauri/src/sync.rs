//! Automatic lyrics offset: slides the lyrics' "someone is singing now" pattern over the
//! separated vocals' loudness and keeps the shift where they line up best. Fixes the common
//! case of a music video with an intro (a constant delay); a pause mid-song isn't handled.
use anyhow::ensure;
use std::path::Path;

const FRAME_S: f64 = 0.1;
const MAX_SHIFT_S: f64 = 15.0;
/// A line counts as sung for at most this long (the next line usually starts sooner).
const MAX_LINE_S: f64 = 8.0;
/// Below this correlation the match is a guess: better to leave the lyrics alone.
const MIN_CORR: f64 = 0.3;

/// Loudness of `file` every 100ms (RMS), decoded by ffmpeg as 8kHz mono.
pub fn activity(ffmpeg: &Path, file: &Path) -> anyhow::Result<Vec<f32>> {
    let out = crate::tools::std_cmd(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-vn", "-f", "f32le", "-ac", "1", "-ar", "8000", "-"])
        .output()?;
    ensure!(out.status.success(), "ffmpeg (sincronia): {}", String::from_utf8_lossy(&out.stderr));
    let samples: Vec<f32> = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
    let per_frame = (8000.0 * FRAME_S) as usize;
    Ok(samples
        .chunks(per_frame)
        .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
        .collect())
}

/// Start times of the .lrc lines and whether each has text (empty lines mark instrumental parts).
pub fn parse_lrc(src: &str) -> Vec<(f64, bool)> {
    let mut out = vec![];
    for line in src.lines() {
        let mut rest = line;
        let mut times = vec![];
        while let Some(tag) = rest.strip_prefix('[').and_then(|r| r.split_once(']')) {
            let (inner, after) = tag;
            if let Some((m, s)) = inner.split_once(':')
                && let (Ok(m), Ok(s)) = (m.parse::<f64>(), s.parse::<f64>())
            {
                times.push(m * 60.0 + s);
            }
            rest = after;
        }
        let has_text = !rest.trim().is_empty();
        out.extend(times.into_iter().map(|t| (t, has_text)));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Offset in ms to add to the lyrics' times (positive = lyrics later), or None when unsure.
pub fn offset_ms(activity: &[f32], lines: &[(f64, bool)]) -> Option<i64> {
    let n = activity.len();
    if n == 0 || !lines.iter().any(|l| l.1) {
        return None;
    }
    // Binary "voice now": the guide voice still carries some instrument bleed, so the threshold
    // is relative to how loud the singing gets (90th percentile), not absolute.
    let mut sorted = activity.to_vec();
    sorted.sort_by(f32::total_cmp);
    let loud = sorted[n * 9 / 10];
    if loud <= 0.0 {
        return None;
    }
    let voice: Vec<f64> = activity.iter().map(|&a| f64::from(a > loud * 0.25)).collect();

    let max_shift = (MAX_SHIFT_S / FRAME_S) as i64;
    let mut expected = vec![0.0; n + 2 * max_shift as usize]; // padded so every shift fits
    for (i, &(t, has_text)) in lines.iter().enumerate() {
        if !has_text {
            continue;
        }
        let end = lines.get(i + 1).map_or(t + MAX_LINE_S, |l| l.0.min(t + MAX_LINE_S));
        let (a, b) = ((t / FRAME_S) as i64 + max_shift, (end / FRAME_S) as i64 + max_shift);
        for f in a.max(0)..b.min(expected.len() as i64) {
            expected[f as usize] = 1.0;
        }
    }

    let corr = |shift: i64| {
        // expected[f - shift] lined up with voice[f]
        let e = |f: usize| expected[(f as i64 - shift + max_shift) as usize];
        let (mut se, mut sv, mut see, mut svv, mut sev) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (f, &v) in voice.iter().enumerate() {
            let x = e(f);
            se += x;
            sv += v;
            see += x * x;
            svv += v * v;
            sev += x * v;
        }
        let k = n as f64;
        let den = ((see - se * se / k) * (svv - sv * sv / k)).sqrt();
        if den == 0.0 { 0.0 } else { (sev - se * sv / k) / den }
    };
    let (best, c) = (-max_shift..=max_shift).map(|s| (s, corr(s))).max_by(|a, b| a.1.total_cmp(&b.1))?;
    (c >= MIN_CORR).then(|| (best as f64 * FRAME_S * 1000.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vocals for `lines` delayed by `delay_s`, with quiet instrument bleed in between.
    fn fake_vocals(lines: &[(f64, bool)], delay_s: f64, len_s: f64) -> Vec<f32> {
        (0..(len_s / FRAME_S) as usize)
            .map(|f| {
                let t = f as f64 * FRAME_S - delay_s;
                let sung = lines.windows(2).any(|w| w[0].1 && t >= w[0].0 && t < w[1].0 - 0.4);
                if sung { 0.3 } else { 0.02 }
            })
            .collect()
    }

    const LRC: &str = "[00:07.08] Tão natural quanto a luz do dia\n[00:10.84] Mas que preguiça boa\n\
        [00:14.88] Hoje ninguém vai estragar meu dia\n[00:18.82] Só vou gastar energia\n[00:23.85]\n\
        [00:30.26] Alguém te perguntou\n[00:34.62] Uma palavra amiga\n[00:38.86] Isso faz falta\n[00:43.00]\n";

    #[test]
    fn parses_lrc_lines() {
        let l = parse_lrc("[ar:Someone]\n[00:01.50]a\n[01:02.00][01:30.25] chorus\n[00:05.00]\n");
        assert_eq!(l, vec![(1.5, true), (5.0, false), (62.0, true), (90.25, true)]);
    }

    #[test]
    fn finds_intro_delay_in_either_direction() {
        let lines = parse_lrc(LRC);
        for delay in [0.0, 2.5, -1.2, 9.0] {
            let got = offset_ms(&fake_vocals(&lines, delay, 60.0), &lines).unwrap();
            assert!((got - (delay * 1000.0) as i64).abs() <= 100, "delay {delay}: got {got}ms");
        }
    }

    #[test]
    fn unsure_when_vocals_dont_match() {
        let lines = parse_lrc(LRC);
        assert_eq!(offset_ms(&vec![0.02; 600], &lines), None); // instrumental only
        assert_eq!(offset_ms(&[], &lines), None);
        assert_eq!(offset_ms(&vec![0.3; 600], &parse_lrc("[00:01.00]\n")), None); // no text
    }
}
