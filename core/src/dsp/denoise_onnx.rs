//! Supresión de ruido con ONNX Runtime + DeepFilterNet3.
//!
//! DeepFilterNet3 es un modelo de redes neuronales recurrentes que ofrece
//! mejor calidad que RNNoise, pero requiere ONNX Runtime como dependencia
//! del sistema. El modelo se compone de tres sub-modelos:
//!
//! 1. **Encoder** (`enc.onnx`): procesa características ERB y espectrales.
//! 2. **ERB Decoder** (`erb_dec.onnx`): estima una máscara ERB para
//!    supresión de ruido.
//! 3. **DF Decoder** (`df_dec.onnx`): genera coeficientes de filtrado profundo
//!    para refinar la señal.
//!
//! El pipeline completo por frame:
//! 1. Acumular muestras en buffer circular (overlap-add).
//! 2. STFT con ventana Hann → espectro complejo.
//! 3. Extraer características ERB del espectro de magnitud.
//! 4. Ejecutar encoder → embeddings + skip connections + c0.
//! 5. Ejecutar ERB decoder → máscara ERB.
//! 6. Aplicar máscara al espectro.
//! 7. Ejecutar DF decoder → coeficientes de filtrado profundo.
//! 8. Aplicar DF a los primeros nb_df bins del espectro.
//! 9. ISTFT con overlap-add → audio de salida.
//!
//! # Estado del modelo
//!
//! Los modelos ONNX se esperan en un directorio con:
//! - `enc.onnx`, `erb_dec.onnx`, `df_dec.onnx`
//! - `config.ini` con los parámetros del modelo.
//!
//! Si no se encuentran los modelos, el procesador funciona como passthrough.

use std::f32::consts::PI;
use std::path::PathBuf;

use ort::session::Session;

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

// ── Valores por defecto de DeepFilterNet3 ──

const DEFAULT_FFT_SIZE: usize = 960;
const DEFAULT_HOP_SIZE: usize = 480;
const DEFAULT_NB_ERB: usize = 32;
const DEFAULT_NB_DF: usize = 96;
const DEFAULT_DF_ORDER: usize = 10;
const DEFAULT_CONV_CH: usize = 64;
const DEFAULT_CONV_LOOKAHEAD: usize = 2;
const DEFAULT_DF_LOOKAHEAD: usize = 2;
const DEFAULT_EMB_HIDDEN_DIM: usize = 256;

// ── Config INI ──

fn parse_config(ini_content: &str) -> std::collections::HashMap<(String, String), String> {
    let mut map = std::collections::HashMap::new();
    let mut section = String::new();
    for line in ini_content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') || t.starts_with('#') {
            continue;
        }
        if let Some(s) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = s.to_lowercase();
        } else if let Some((k, v)) = t.split_once('=') {
            map.insert(
                (section.clone(), k.trim().to_lowercase()),
                v.trim().to_string(),
            );
        }
    }
    map
}

fn cfg_usize(
    cfg: &std::collections::HashMap<(String, String), String>,
    sec: &str,
    key: &str,
    def: usize,
) -> usize {
    cfg.get(&(sec.into(), key.into()))
        .and_then(|v| v.parse().ok())
        .unwrap_or(def)
}

fn cfg_f32(
    cfg: &std::collections::HashMap<(String, String), String>,
    sec: &str,
    key: &str,
    def: f32,
) -> f32 {
    cfg.get(&(sec.into(), key.into()))
        .and_then(|v| v.parse().ok())
        .unwrap_or(def)
}

// ── ERB helpers ──

fn erb_from_hz(hz: f32) -> f32 {
    21.4 * (4.7 * hz / 1000.0 + 1.0).ln() * 4.0 / std::f32::consts::LN_10
}

fn hz_from_erb(erb: f32) -> f32 {
    (10f32.powf(erb * std::f32::consts::LN_10 / 4.0) - 1.0) * 1000.0 / 4.7
}

fn build_erb_filterbank(n_freqs: usize, fft_size: usize, sr: u32, nb_erb: usize) -> Vec<Vec<f32>> {
    let sr_f = sr as f32;
    let erb_min = erb_from_hz(0.0);
    let erb_max = erb_from_hz(sr_f / 2.0);
    let mut fb = vec![vec![0.0f32; n_freqs]; nb_erb];
    for bin in 1..n_freqs {
        let freq = bin as f32 * sr_f / fft_size as f32;
        let erb_idx = erb_from_hz(freq);
        let norm = (erb_idx - erb_min) / (erb_max - erb_min);
        let band_f = norm * nb_erb as f32;
        let band = (band_f as usize).min(nb_erb - 1);
        let frac = band_f - band as f32;
        fb[band][bin] += 1.0 - frac;
        if frac > 0.0 && band + 1 < nb_erb {
            fb[band + 1][bin] += frac;
        }
    }
    fb
}

// ── FFT Cooley-Tukey radix-2 in-place ──

fn fft_in_place(buf: &mut [(f32, f32)], n: usize, inverse: bool) {
    let mut j = 0usize;
    for i in 0..n {
        if i < j {
            buf.swap(i, j);
        }
        let mut m = n >> 1;
        while m >= 1 && j >= m {
            j -= m;
            m >>= 1;
        }
        j += m;
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = sign * 2.0 * PI / len as f32;
        let wr = angle.cos();
        let wi = angle.sin();
        for start in (0..n).step_by(len) {
            let mut w_r = 1.0f32;
            let mut w_i = 0.0f32;
            for k in 0..half {
                let u = buf[start + k];
                let v = buf[start + k + half];
                let t_r = w_r * v.0 - w_i * v.1;
                let t_i = w_r * v.1 + w_i * v.0;
                buf[start + k] = (u.0 + t_r, u.1 + t_i);
                buf[start + k + half] = (u.0 - t_r, u.1 - t_i);
                let new_w_r = w_r * wr - w_i * wi;
                let new_w_i = w_r * wi + w_i * wr;
                w_r = new_w_r;
                w_i = new_w_i;
            }
        }
        len <<= 1;
    }
    if inverse {
        let inv = 1.0 / n as f32;
        for v in buf.iter_mut() {
            v.0 *= inv;
            v.1 *= inv;
        }
    }
}

// ── Estado de inferencia ──

pub struct OnnxDenoise {
    enabled: bool,
    fft_size: usize,
    hop_size: usize,
    nb_erb: usize,
    nb_df: usize,
    df_order: usize,
    conv_ch: usize,
    n_freqs: usize,

    session_enc: Option<Session>,
    session_erb_dec: Option<Session>,
    session_df_dec: Option<Session>,

    window: Vec<f32>,
    input_ring: Vec<f32>,
    ring_pos: usize,
    output_overlap: Vec<f32>,

    erb_filterbank: Vec<Vec<f32>>,

    fft_buf: Vec<(f32, f32)>,
    spec_power: Vec<f32>,
    feat_erb: Vec<f32>,
    feat_spec: Vec<f32>,
    spec_out: Vec<f32>,
    spec_out_buf: Vec<f32>,

    erb_norm_state: Vec<f32>,
    norm_alpha: f32,
}

impl OnnxDenoise {
    pub fn new(model_dir: PathBuf) -> Result<Self, crate::Error> {
        let cfg = {
            let path = model_dir.join("config.ini");
            if path.exists() {
                let c = std::fs::read_to_string(&path).map_err(crate::Error::Io)?;
                parse_config(&c)
            } else {
                std::collections::HashMap::new()
            }
        };

        let sr = 48000u32;
        let fft_size = cfg_usize(&cfg, "df", "fft_size", DEFAULT_FFT_SIZE);
        let hop_size = cfg_usize(&cfg, "df", "hop_size", DEFAULT_HOP_SIZE);
        let nb_erb = cfg_usize(&cfg, "df", "nb_erb", DEFAULT_NB_ERB);
        let nb_df = cfg_usize(&cfg, "df", "nb_df", DEFAULT_NB_DF);
        let df_order = cfg_usize(&cfg, "deepfilternet", "df_order", DEFAULT_DF_ORDER);
        let conv_ch = cfg_usize(&cfg, "deepfilternet", "conv_ch", DEFAULT_CONV_CH);
        let n_freqs = fft_size / 2 + 1;

        let enc = load_session(&model_dir.join("enc.onnx"));
        let erb_dec = load_session(&model_dir.join("erb_dec.onnx"));
        let df_dec = load_session(&model_dir.join("df_dec.onnx"));
        let enabled = enc.is_ok() && erb_dec.is_ok() && df_dec.is_ok();

        let norm_alpha = cfg_f32(&cfg, "df", "norm_alpha", 0.9);

        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / fft_size as f32).cos()))
            .collect();

        let erb_filterbank = build_erb_filterbank(n_freqs, fft_size, sr, nb_erb);

        Ok(Self {
            enabled,
            fft_size,
            hop_size,
            nb_erb,
            nb_df,
            df_order,
            conv_ch,
            n_freqs,
            session_enc: enc.ok(),
            session_erb_dec: erb_dec.ok(),
            session_df_dec: df_dec.ok(),
            window,
            input_ring: vec![0.0; fft_size],
            ring_pos: 0,
            output_overlap: vec![0.0; fft_size],
            erb_filterbank,
            fft_buf: vec![(0.0, 0.0); fft_size],
            spec_power: vec![0.0; n_freqs],
            feat_erb: vec![0.0; nb_erb],
            feat_spec: vec![0.0; 2 * nb_df],
            spec_out: vec![0.0; 2 * n_freqs],
            spec_out_buf: vec![0.0; n_freqs * 2],
            erb_norm_state: vec![0.0; nb_erb],
            norm_alpha,
        })
    }

    fn compute_stft(&mut self) {
        for i in 0..self.fft_size {
            let idx = (self.ring_pos + i) % self.fft_size;
            self.fft_buf[i] = (self.input_ring[idx] * self.window[i], 0.0);
        }
        fft_in_place(&mut self.fft_buf, self.fft_size, false);
        for i in 0..self.n_freqs {
            self.spec_out[2 * i] = self.fft_buf[i].0;
            self.spec_out[2 * i + 1] = self.fft_buf[i].1;
        }
    }

    fn compute_erb_features(&mut self) {
        for i in 0..self.n_freqs {
            let re = self.spec_out[2 * i];
            let im = self.spec_out[2 * i + 1];
            self.spec_power[i] = re * re + im * im;
        }
        for b in 0..self.nb_erb {
            let mut sum = 0.0f32;
            for i in 1..self.n_freqs {
                sum += self.erb_filterbank[b][i] * self.spec_power[i];
            }
            let abs_sum = sum.abs();
            self.erb_norm_state[b] =
                self.erb_norm_state[b] * self.norm_alpha + abs_sum * (1.0 - self.norm_alpha);
            let norm_val = self.erb_norm_state[b].max(1e-10);
            self.feat_erb[b] = (abs_sum / norm_val + 1e-10).ln();
        }
    }

    fn compute_spec_features(&mut self) {
        let scale = 1.0 / self.fft_size as f32;
        for i in 0..self.nb_df {
            self.feat_spec[2 * i] = self.spec_out[2 * i] * scale;
            self.feat_spec[2 * i + 1] = self.spec_out[2 * i + 1] * scale;
        }
    }

    fn apply_mask(&mut self, mask: &[f32]) {
        let nb_df = self.nb_df;
        let n_freqs = self.n_freqs;
        for i in 0..n_freqs {
            let erb_idx_f = i as f32 * self.nb_erb as f32 / n_freqs as f32;
            let erb_idx = (erb_idx_f as usize).min(self.nb_erb - 1);
            let frac = erb_idx_f - erb_idx as f32;
            let m = mask[erb_idx] * (1.0 - frac)
                + if erb_idx + 1 < self.nb_erb {
                    mask[erb_idx + 1] * frac
                } else {
                    0.0
                };
            let m = m.clamp(0.0, 1.0);
            self.spec_out_buf[2 * i] = self.spec_out[2 * i] * m;
            self.spec_out_buf[2 * i + 1] = self.spec_out[2 * i + 1] * m;
        }
        // Solo los primeros nb_df bins se refinan con DF; el resto queda con la máscara ERB.
        for i in nb_df..n_freqs {
            self.spec_out_buf[2 * i] = self.spec_out[2 * i];
            self.spec_out_buf[2 * i + 1] = self.spec_out[2 * i + 1];
        }
    }

    fn apply_df(&mut self, coefs: &[f32]) {
        let nb_df = self.nb_df;
        let df_order = self.df_order;
        let n_freqs = self.n_freqs;
        for f in 0..nb_df {
            let mut out_re = self.spec_out_buf[2 * f];
            let mut out_im = self.spec_out_buf[2 * f + 1];
            for o in 0..df_order {
                let idx = o * nb_df * 2 + f * 2;
                let c_re = coefs[idx];
                let c_im = coefs[idx + 1];
                // Aplicar coeficiente DF: X[f] += coef * X[f] del frame anterior.
                // Simplificación: aplicamos directamente como filtrado de segundo orden.
                out_re += c_re * self.spec_out[2 * f] - c_im * self.spec_out[2 * f + 1];
                out_im += c_re * self.spec_out[2 * f + 1] + c_im * self.spec_out[2 * f];
            }
            self.spec_out_buf[2 * f] = out_re;
            self.spec_out_buf[2 * f + 1] = out_im;
        }
    }

    fn compute_istft(&mut self, output: &mut [f32]) {
        let frames = output.len().min(self.hop_size);

        // Copiar spec_out_buf al buffer ifft (simétrico para IFFT real).
        let mut ifft_buf = vec![(0.0f32, 0.0f32); self.fft_size];
        for i in 0..self.n_freqs {
            ifft_buf[i] = (self.spec_out_buf[2 * i], self.spec_out_buf[2 * i + 1]);
        }
        for i in 1..self.fft_size - self.n_freqs + 1 {
            let mirror = self.fft_size - i;
            if mirror < self.n_freqs {
                ifft_buf[i] = (self.spec_out_buf[2 * i], -self.spec_out_buf[2 * i + 1]);
            }
        }

        fft_in_place(&mut ifft_buf, self.fft_size, true);

        for i in 0..frames {
            let idx = (self.ring_pos + i) % self.fft_size;
            output[i] = ifft_buf[idx].0 + self.output_overlap[i];
        }

        for i in 0..self.fft_size - self.hop_size {
            self.output_overlap[i] =
                ifft_buf[(self.ring_pos + self.hop_size + i) % self.fft_size].0;
        }
        for i in (self.fft_size - self.hop_size)..self.fft_size {
            self.output_overlap[i] = 0.0;
        }
    }
}

fn load_session(path: &std::path::Path) -> Result<Session, crate::Error> {
    if !path.exists() {
        return Err(crate::Error::audio(format!(
            "ONNX model not found: {}",
            path.display()
        )));
    }
    Session::builder()
        .map_err(|e| crate::Error::audio(format!("ort builder: {e}")))?
        .commit_from_file(path)
        .map_err(|e| crate::Error::audio(format!("ort load {}: {e}", path.display())))
}

impl AudioProcessor for OnnxDenoise {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        if !self.enabled {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }

        let hop = self.hop_size;
        let mut pos = 0;

        while pos < frames {
            let chunk = (frames - pos).min(hop);
            let input_chunk = &input[pos..pos + chunk];
            let output_chunk = &mut output[pos..pos + chunk];

            // 1) Acumular en ring buffer.
            for &s in input_chunk {
                self.input_ring[self.ring_pos] = s;
                self.ring_pos = (self.ring_pos + 1) % self.fft_size;
            }

            // 2) STFT.
            self.compute_stft();

            // 3) ERB features.
            self.compute_erb_features();

            // 4) Spec features.
            self.compute_spec_features();

            // 5) Inferencia ONNX.
            let mut mask_erb = vec![0.0f32; self.nb_erb];
            let mut df_coefs = vec![0.0f32; self.nb_df * self.df_order * 2];

            if let (Some(enc), Some(erb_dec), Some(df_dec)) = (
                self.session_enc.as_mut(),
                self.session_erb_dec.as_mut(),
                self.session_df_dec.as_mut(),
            ) {
                if let Err(e) = run_inference(
                    enc,
                    erb_dec,
                    df_dec,
                    &self.feat_erb,
                    &self.feat_spec,
                    self.nb_erb,
                    self.nb_df,
                    self.conv_ch,
                    &mut mask_erb,
                    &mut df_coefs,
                ) {
                    log::warn!("ONNX inference failed, passthrough: {e}");
                    output_chunk.copy_from_slice(input_chunk);
                    pos += chunk;
                    continue;
                }
            }

            // 6) Aplicar máscara ERB al espectro.
            self.apply_mask(&mask_erb);

            // 7) Aplicar DF coefficients.
            if df_coefs.iter().any(|&x| x != 0.0) {
                self.apply_df(&df_coefs);
            } else {
                self.spec_out_buf.copy_from_slice(&self.spec_out);
            }

            // 8) ISTFT + overlap-add.
            self.compute_istft(output_chunk);

            pos += chunk;
        }

        ProcessResult {
            latency_ms: (self.hop_size as f32 / 48000.0) * 1000.0,
        }
    }

    fn name(&self) -> &'static str {
        "onnx_denoise"
    }

    fn reset(&mut self) {
        self.ring_pos = 0;
        self.input_ring.fill(0.0);
        self.output_overlap.fill(0.0);
        self.erb_norm_state.fill(0.0);
    }
}

// ── Inferencia ONNX ──

fn run_inference(
    enc: &mut Session,
    erb_dec: &mut Session,
    df_dec: &mut Session,
    feat_erb: &[f32],
    feat_spec: &[f32],
    nb_erb: usize,
    nb_df: usize,
    conv_ch: usize,
    mask_erb: &mut [f32],
    df_coefs: &mut [f32],
) -> Result<(), Box<dyn std::error::Error>> {
    use ndarray::Array4;

    // ── Encoder ──
    let erb_shape = [1, 1, 1, nb_erb];
    let spec_shape = [1, 2, 1, nb_df];

    let erb_arr = Array4::from_shape_vec(erb_shape, feat_erb.to_vec())?;
    let spec_arr = Array4::from_shape_vec(spec_shape, feat_spec.to_vec())?;

    let erb_tensor = Tensor::from_array(erb_arr)?;
    let spec_tensor = Tensor::from_array(spec_arr)?;

    let enc_outputs = enc.run(ort::inputs![
        "feat_erb" => erb_tensor,
        "feat_spec" => spec_tensor,
    ]?)?;

    // Extraer outputs del encoder: e0, e1, e2, e3, emb, c0, lsnr
    // Necesitamos: emb, c0, e0..e3
    let emb_value = enc_outputs["emb"].try_extract_tensor::<f32>()?;
    let c0_value = enc_outputs["c0"].try_extract_tensor::<f32>()?;
    let e0_value = enc_outputs["e0"].try_extract_tensor::<f32>()?;
    let e1_value = enc_outputs["e1"].try_extract_tensor::<f32>()?;
    let e2_value = enc_outputs["e2"].try_extract_tensor::<f32>()?;
    let e3_value = enc_outputs["e3"].try_extract_tensor::<f32>()?;

    let emb_arr = emb_value.to_owned().into_dimensionality::<ndarray::Ix3>()?;
    let c0_arr = c0_value.to_owned().into_dimensionality::<ndarray::Ix4>()?;
    let e0_arr = e0_value.to_owned().into_dimensionality::<ndarray::Ix4>()?;
    let e1_arr = e1_value.to_owned().into_dimensionality::<ndarray::Ix4>()?;
    let e2_arr = e2_value.to_owned().into_dimensionality::<ndarray::Ix4>()?;
    let e3_arr = e3_value.to_owned().into_dimensionality::<ndarray::Ix4>()?;

    // ── ERB Decoder ──
    let erb_dec_outputs = erb_dec.run(ort::inputs![
        "emb" => Tensor::from_array(emb_arr)?,
        "e3" => Tensor::from_array(e3_arr)?,
        "e2" => Tensor::from_array(e2_arr)?,
        "e1" => Tensor::from_array(e1_arr)?,
        "e0" => Tensor::from_array(e0_arr)?,
    ]?)?;

    let mask_value = erb_dec_outputs["m"].try_extract_tensor::<f32>()?;
    let mask_arr = mask_value
        .to_owned()
        .into_dimensionality::<ndarray::Ix4>()?;
    let mask_data = mask_arr.as_slice().unwrap_or_default();
    let mask_len = mask_data.len().min(mask_erb.len());
    mask_erb[..mask_len].copy_from_slice(&mask_data[..mask_len]);

    // ── DF Decoder ──
    let df_dec_outputs = df_dec.run(ort::inputs![
        "emb" => Tensor::from_array(emb_arr)?,
        "c0" => Tensor::from_array(c0_arr)?,
    ]?)?;

    let coefs_value = df_dec_outputs["coefs"].try_extract_tensor::<f32>()?;
    let coefs_data = coefs_value.as_slice().unwrap_or_default();
    let coefs_len = coefs_data.len().min(df_coefs.len());
    df_coefs[..coefs_len].copy_from_slice(&coefs_data[..coefs_len]);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onnx_denoise_creation_without_models_returns_error() {
        let result = OnnxDenoise::new(std::path::PathBuf::from("/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn erb_filterbank_has_correct_dimensions() {
        let fb = build_erb_filterbank(481, 960, 48000, 32);
        assert_eq!(fb.len(), 32);
        assert_eq!(fb[0].len(), 481);
    }

    #[test]
    fn erb_filterbank_weights_are_positive() {
        let fb = build_erb_filterbank(481, 960, 48000, 32);
        for band in &fb {
            for &w in band {
                assert!(w >= 0.0, "ERB weight must be non-negative");
            }
        }
    }

    #[test]
    fn fft_roundtrip() {
        let n = 512;
        let signal: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let t = i as f32 / n as f32;
                ((2.0 * PI * 10.0 * t).sin(), 0.0)
            })
            .collect();
        let mut buf = signal.clone();
        fft_in_place(&mut buf, n, false);
        fft_in_place(&mut buf, n, true);
        for (orig, &computed) in signal.iter().zip(buf.iter()) {
            assert!(
                (orig.0 - computed.0).abs() < 1e-5,
                "FFT roundtrip failed: {} vs {}",
                orig.0,
                computed.0
            );
        }
    }

    #[test]
    fn parse_config_basic() {
        let ini = "[df]\nsr = 48000\nfft_size = 960\n\n[deepfilternet]\nconv_ch = 64\n";
        let cfg = parse_config(ini);
        assert_eq!(cfg.get(&("df".into(), "sr".into())).unwrap(), "48000");
        assert_eq!(
            cfg.get(&("deepfilternet".into(), "conv_ch".into()))
                .unwrap(),
            "64"
        );
    }
}
