//! Lyrics from LRCLIB (free, no key). Prefers time-synced (.lrc) lyrics.
use crate::tools::http;
use serde::Deserialize;

const API: &str = "https://lrclib.net/api";
const MAX_DURATION_DIFF: f64 = 10.0;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    #[serde(default)]
    duration: f64,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Returns (lyrics, is_synced).
pub async fn fetch(
    title: &str,
    artist: Option<&str>,
    track: Option<&str>,
    duration: f64,
) -> anyhow::Result<Option<(String, bool)>> {
    if let (Some(artist), Some(track)) = (artist, track) {
        let resp = http()
            .get(format!("{API}/get"))
            .query(&[("artist_name", artist), ("track_name", track), ("duration", &duration.round().to_string())])
            .send()
            .await?;
        if resp.status().is_success() {
            if let Some(found) = best(&[resp.json::<Track>().await?], duration) {
                return Ok(Some(found));
            }
        }
    }
    let results: Vec<Track> = http()
        .get(format!("{API}/search"))
        .query(&[("q", clean_title(title))])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(best(&results, duration))
}

/// Closest duration within ±10s; synced lyrics beat plain ones.
fn best(results: &[Track], duration: f64) -> Option<(String, bool)> {
    let close = |t: &&Track| (t.duration - duration).abs() <= MAX_DURATION_DIFF;
    let by_diff = |a: &&Track, b: &&Track| (a.duration - duration).abs().total_cmp(&(b.duration - duration).abs());
    if let Some(t) = results.iter().filter(close).filter(|t| t.synced_lyrics.is_some()).min_by(by_diff) {
        return Some((t.synced_lyrics.clone()?, true));
    }
    let t = results.iter().filter(close).filter(|t| t.plain_lyrics.is_some()).min_by(by_diff)?;
    Some((t.plain_lyrics.clone()?, false))
}

/// "Queen – Bohemian Rhapsody (Official Video Remastered) | HD" → "Queen Bohemian Rhapsody"
pub fn clean_title(title: &str) -> String {
    let mut s = title.split('|').next().unwrap_or(title).to_string();
    for (open, close) in [('(', ')'), ('[', ']'), ('【', '】')] {
        while let (Some(a), Some(b)) = (s.find(open), s.find(close)) {
            if b < a {
                break;
            }
            s.replace_range(a..b + close.len_utf8(), " ");
        }
    }
    let lower = s.to_lowercase();
    // byte offsets from `lower` are only valid on `s` when lowercasing kept the length
    let same_len = |l: &str, s: &str| l.len() == s.len();
    for cut in [" ft.", " ft ", " feat.", " feat ", " featuring "] {
        if let Some(i) = lower.find(cut).filter(|_| same_len(&lower, &s)) {
            s.truncate(i);
            break;
        }
    }
    const NOISE: [&str; 9] =
        ["official music video", "official video", "official audio", "lyric video", "lyrics", "letra", "legendado", "videoclipe", "clipe oficial"];
    let mut lower = s.to_lowercase();
    for n in NOISE {
        while same_len(&lower, &s)
            && let Some(i) = lower.find(n)
        {
            s.replace_range(i..i + n.len(), " ");
            lower.replace_range(i..i + n.len(), " ");
        }
    }
    s.split(|c: char| c.is_whitespace() || c == '-' || c == '–' || c == '"')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_youtube_titles() {
        let cases = [
            ("Queen – Bohemian Rhapsody (Official Video Remastered)", "Queen Bohemian Rhapsody"),
            ("Marília Mendonça - Supera [Clipe Oficial]", "Marília Mendonça Supera"),
            ("Ed Sheeran - Perfect (Official Music Video) | HD", "Ed Sheeran Perfect"),
            ("Shakira - Waka Waka ft. Freshlyground", "Shakira Waka Waka"),
            ("Evidências - Chitãozinho & Xororó (Letra)", "Evidências Chitãozinho & Xororó"),
            ("Adele - Hello Lyrics", "Adele Hello"),
            ("Nirvana - Smells Like Teen Spirit", "Nirvana Smells Like Teen Spirit"),
        ];
        for (input, want) in cases {
            assert_eq!(clean_title(input), want, "input: {input}");
        }
    }

    fn t(duration: f64, plain: bool, synced: bool) -> Track {
        Track {
            duration,
            plain_lyrics: plain.then(|| format!("plain {duration}")),
            synced_lyrics: synced.then(|| format!("synced {duration}")),
        }
    }

    #[test]
    fn picks_synced_closest_in_range() {
        let r = [t(200.0, true, true), t(181.0, true, true), t(180.0, true, false), t(400.0, true, true)];
        assert_eq!(best(&r, 182.0), Some(("synced 181".into(), true)));
        assert_eq!(best(&[t(180.0, true, false)], 182.0), Some(("plain 180".into(), false)));
        assert_eq!(best(&[t(300.0, true, true)], 182.0), None);
    }
}
