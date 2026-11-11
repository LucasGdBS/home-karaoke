//! Vocal removal with UVR's MDX-Net (ONNX). Mirrors UVR's MDX inference:
//! STFT → model predicts the instrumental spectrogram → iSTFT, in overlapping chunks.
use anyhow::Context;
use ort::{session::Session, value::Tensor};
use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use std::sync::Arc;

// Must match the model's config (UVR model_data.json for UVR-MDX-NET-Inst_HQ_3:
// mdx_n_fft_scale_set 6144, mdx_dim_f_set 3072, mdx_dim_t_set 8 → 2^8 frames).
const N_FFT: usize = 6144;
const HOP: usize = 1024;
const DIM_F: usize = 3072;
const DIM_T: usize = 256;
const CHUNK: usize = HOP * (DIM_T - 1);
const TRIM: usize = N_FFT / 2;
const GEN: usize = CHUNK - 2 * TRIM; // useful samples per chunk (edges are discarded)

struct Stft {
    fwd: Arc<dyn Fft<f32>>,
    inv: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
}

impl Stft {
    fn new() -> Self {
        let mut planner = FftPlanner::new();
        Stft {
            fwd: planner.plan_fft_forward(N_FFT),
            inv: planner.plan_fft_inverse(N_FFT),
            // periodic Hann, like torch.hann_window
            window: (0..N_FFT)
                .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N_FFT as f32).cos())
                .collect(),
        }
    }

    /// torch.stft(center=True, reflect pad) of a CHUNK-long signal, truncated to DIM_F bins.
    /// Output layout: [f][t].
    fn forward(&self, x: &[f32]) -> Vec<Complex32> {
        assert_eq!(x.len(), CHUNK);
        let reflect = |j: isize| -> f32 {
            let n = x.len() as isize;
            let j = if j < 0 { -j } else if j >= n { 2 * (n - 1) - j } else { j };
            x[j as usize]
        };
        let mut spec = vec![Complex32::default(); DIM_F * DIM_T];
        let mut buf = vec![Complex32::default(); N_FFT];
        for t in 0..DIM_T {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = Complex32::new(reflect((t * HOP + i) as isize - TRIM as isize) * self.window[i], 0.0);
            }
            self.fwd.process(&mut buf);
            for f in 0..DIM_F {
                spec[f * DIM_T + t] = buf[f];
            }
        }
        spec
    }

    /// Inverse of `forward` (missing Nyquist bin = 0), returns CHUNK samples.
    fn inverse(&self, spec: &[Complex32]) -> Vec<f32> {
        let mut out = vec![0f32; CHUNK + N_FFT];
        let mut wsum = vec![0f32; CHUNK + N_FFT];
        let mut buf = vec![Complex32::default(); N_FFT];
        for t in 0..DIM_T {
            for (k, b) in buf.iter_mut().enumerate() {
                *b = match k {
                    k if k < DIM_F => spec[k * DIM_T + t],
                    k if k == N_FFT / 2 => Complex32::default(),
                    k => spec[(N_FFT - k) * DIM_T + t].conj(),
                };
            }
            self.inv.process(&mut buf);
            for i in 0..N_FFT {
                let w = self.window[i];
                out[t * HOP + i] += buf[i].re / N_FFT as f32 * w;
                wsum[t * HOP + i] += w * w;
            }
        }
        (TRIM..TRIM + CHUNK)
            .map(|i| if wsum[i] > 1e-8 { out[i] / wsum[i] } else { 0.0 })
            .collect()
    }
}

/// `mix`: interleaved stereo f32 at 44.1kHz. Returns the interleaved instrumental.
pub fn separate(session: &mut Session, mix: &[f32], mut on_progress: impl FnMut(f32)) -> anyhow::Result<Vec<f32>> {
    let n = mix.len() / 2;
    // pad so every chunk window [i, i+CHUNK) is in bounds and covers samples i..i+GEN
    let pad = |ch: usize| -> Vec<f32> {
        let mut v = vec![0f32; TRIM];
        v.extend(mix.chunks_exact(2).map(|s| s[ch]));
        v.resize(TRIM + n + TRIM + GEN, 0.0);
        v
    };
    let chans = [pad(0), pad(1)];
    let stft = Stft::new();
    let input_name = session.inputs[0].name.clone();
    let mut out = [Vec::with_capacity(n + GEN), Vec::with_capacity(n + GEN)];
    let starts: Vec<usize> = (0..n).step_by(GEN).collect();

    for (done, &i) in starts.iter().enumerate() {
        let mut data = vec![0f32; 4 * DIM_F * DIM_T];
        let plane = DIM_F * DIM_T;
        for (c, ch) in chans.iter().enumerate() {
            // channel order [L.re, L.im, R.re, R.im], like UVR's view_as_real + reshape
            for (k, z) in stft.forward(&ch[i..i + CHUNK]).iter().enumerate() {
                data[2 * c * plane + k] = z.re;
                data[(2 * c + 1) * plane + k] = z.im;
            }
        }
        let tensor = Tensor::from_array(([1usize, 4, DIM_F, DIM_T], data))?;
        let outputs = session.run(ort::inputs![input_name.as_str() => tensor])?;
        let (_, pred) = outputs[0].try_extract_tensor::<f32>().context("saída do modelo")?;
        for (c, o) in out.iter_mut().enumerate() {
            let spec: Vec<Complex32> = (0..plane)
                .map(|k| Complex32::new(pred[2 * c * plane + k], pred[(2 * c + 1) * plane + k]))
                .collect();
            o.extend_from_slice(&stft.inverse(&spec)[TRIM..TRIM + GEN]);
        }
        on_progress((done + 1) as f32 / starts.len() as f32);
    }
    Ok((0..n).flat_map(|i| [out[0][i], out[1][i]]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stft_roundtrip_reconstructs_signal() {
        let x: Vec<f32> = (0..CHUNK)
            .map(|i| {
                let t = i as f32 / 44100.0;
                0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() + 0.2 * (2.0 * std::f32::consts::PI * 3000.0 * t).sin()
            })
            .collect();
        let s = Stft::new();
        let y = s.inverse(&s.forward(&x));
        let max_err = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(max_err < 1e-3, "max error {max_err}");
    }
}
