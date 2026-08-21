// Asesor de configuración con IA (Groq): envía métricas de voz + estado DSP
// a un LLM y devuelve sugerencias ultra-específicas de ajuste.
//
// El prompt incluye el estado actual de la cadena DSP para que el LLM
// recomiende exactamente qué control mover y a qué valor.

use voxlfa_core::protocol::{
    AnalysisSample, DspState, Suggestion, SuggestionAction, SuggestionKind, VoiceMetrics,
};

/// Endpoint de la API de Groq (OpenAI-compatible).
const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Modelo por defecto en Groq.
const GROQ_MODEL: &str = "openai/gpt-oss-20b";

/// Timeout de la petición LLM en segundos.
const LLM_TIMEOUT_SECS: u64 = 30;

/// Número máximo de sugerencias que esperamos del LLM.
const MAX_SUGGESTIONS: usize = 5;

/// Reintentos ante rate-limit (429).
const MAX_RETRIES: u32 = 2;

/// Construye el prompt del sistema con instrucciones ultra-específicas.
fn system_prompt() -> String {
    "Eres el asistente de VoxLFA (procesador vocal). Responde SOLO JSON array.\n\
     Cada objeto: {id:int, severity:0-1, message:str, action:{type,...}}\n\
     Actions: none | applyPreset{preset:dry|vozLimpia|radio|warm} |\n\
     setEqBand{bandIndex:0-6,gainDb:-18..18} | setDenoise{mix:0-1} |\n\
     setFeedback{thresholdDb:-60..-10,q:2..30} | setPitchCorrection{strength:0-1,mix:0-1} |\n\
     setNoiseGate{thresholdDb:-60..-10,rangeDb:0..60}\n\
     EQ: 0=200Hz,1=500Hz,2=1kHz,3=2kHz,4=3kHz,5=5kHz,6=8kHz\n\
     message: instrucción completa que empieza con el nombre del panel.\n\
     Ej: \"En Ecualizador: baja la Banda 5 (5kHz) a -2 dB para reducir sibilancia\"\n\
     Paneles: Ecualizador, Puerta de ruido, Supresión de ruido, Antifeedback,\n\
     Corrección de tono, Presets\n\
     severity>=0.75 critico, >=0.4 recomendado, <0.4 opcional. Si todo OK: []."
        .to_string()
}

/// Construye el prompt del usuario con las métricas actuales y el estado DSP.
fn user_prompt(metrics: &VoiceMetrics, dsp: Option<&DspState>) -> String {
    let mut text = format!(
        "MÉTRICAS DE VOZ ACTUALES:\n\
         - RMS: {rms:.1} dBFS\n\
         - Pico: {peak:.1} dBFS\n\
         - Rango dinámico: {dr:.1} dB\n\
         - Cresta: {crest:.1} dB\n\
         - Brillo (0–1): {brightness:.3}\n\
         - Resonancia baja-media (0–1): {resonance:.3}\n\
         - Fatiga vocal (0–1): {fatigue:.3}\n\n",
        rms = metrics.rms_db,
        peak = metrics.peak_db,
        dr = metrics.dynamic_range_db,
        crest = metrics.crest_db,
        brightness = metrics.brightness,
        resonance = metrics.resonance_score,
        fatigue = metrics.fatigue_score,
    );

    if let Some(dsp) = dsp {
        text.push_str("ESTADO ACTUAL DE LA CADENA DSP:\n");
        text.push_str(&format!("- Preset activo: {}\n", dsp.preset));
        text.push_str(&format!(
            "- Bypass global: {}\n",
            if dsp.global_bypass {
                "SÍ (todo desactivado)"
            } else {
                "NO"
            }
        ));
        for link in &dsp.links {
            if !link.enabled || link.bypass {
                continue;
            }
            if let Some(ref bands) = link.eq_bands {
                text.push_str("- EQ bandas: ");
                for (i, band) in bands.iter().enumerate() {
                    text.push_str(&format!(
                        "[{}: {} Hz, {:+.1} dB, Q={:.1}] ",
                        i, band.freq_hz as u32, band.gain_db, band.q
                    ));
                }
                text.push('\n');
            }
            if let Some(ref params) = link.gate_params {
                text.push_str(&format!(
                    "- NoiseGate: threshold={:.0} dB, range={:.0} dB\n",
                    params.threshold_db, params.range_db
                ));
            }
            if let Some(ref params) = link.denoise_params {
                text.push_str(&format!("- Denoise: mix={:.0}%\n", params.mix * 100.0));
            }
            if let Some(ref params) = link.feedback_params {
                text.push_str(&format!(
                    "- Feedback: threshold={:.0} dB, Q={:.0}\n",
                    params.threshold_db, params.q
                ));
            }
            if let Some(ref params) = link.pitch_correction_params {
                text.push_str(&format!(
                    "- PitchCorrection: strength={:.0}%, mix={:.0}%, scale={}, root={}\n",
                    params.strength * 100.0,
                    params.mix * 100.0,
                    serde_json::to_string(&params.scale).unwrap_or_default(),
                    serde_json::to_string(&params.root).unwrap_or_default(),
                ));
            }
        }
        text.push('\n');
    }

    text.push_str(
        "Analiza las métricas y el estado actual. Recomienda los ajustes más útiles.\n\
         Si recomiendas un cambio, indica EXACTAMENTE qué valor poner.\n\
         No repitas ajustes que ya están en el valor correcto.",
    );
    text
}

/// Petición a la API de Groq (formato OpenAI Chat Completions).
#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(serde::Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Respuesta de la API de Groq (solo extraemos el contenido del primer choice).
#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: String,
}

/// Sugerencia raw parseada del JSON del LLM (campos en camelCase).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSuggestion {
    id: u8,
    severity: f32,
    message: String,
    action: RawAction,
}

/// Acción raw del LLM (tagged union por `type`).
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum RawAction {
    #[serde(rename_all = "camelCase")]
    None,
    #[serde(rename_all = "camelCase")]
    ApplyPreset { preset: String },
    #[serde(rename_all = "camelCase")]
    SetEqBand { band_index: u8, gain_db: f32 },
    #[serde(rename_all = "camelCase")]
    SetDenoise { mix: f32 },
    #[serde(rename_all = "camelCase")]
    SetFeedback { threshold_db: f32, q: f32 },
    #[serde(rename_all = "camelCase")]
    SetPitchCorrection { strength: f32, mix: f32 },
    #[serde(rename_all = "camelCase")]
    SetNoiseGate { threshold_db: f32, range_db: f32 },
}

/// Resultado de una petición al asesor de IA.
#[derive(Debug, Clone)]
pub struct AiAdvisorResult {
    /// Sugerencias generadas por el LLM (vacío si la voz está equilibrada).
    pub suggestions: Vec<Suggestion>,
    /// Mensaje de error si la petición falló (vacío si fue exitosa).
    pub error: String,
}

/// Pide consejo al LLM con las métricas y el estado DSP dados.
///
/// `api_key` es la clave de la API de Groq. Si está vacía o la petición falla,
/// devuelve un [`AiAdvisorResult`] con `error` no vacío.
pub fn request_suggestions(
    api_key: &str,
    analysis: &AnalysisSample,
    dsp: Option<&DspState>,
) -> AiAdvisorResult {
    if api_key.is_empty() {
        return AiAdvisorResult {
            suggestions: Vec::new(),
            error: "No hay clave de API de Groq configurada.".into(),
        };
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(LLM_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: format!("Error al crear cliente HTTP: {e}"),
            };
        }
    };

    let request = ChatRequest {
        model: GROQ_MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_prompt(&analysis.metrics, dsp),
            },
        ],
        temperature: 0.3,
        max_tokens: 1024,
    };

    // Reintento con backoff para rate-limit (429).
    let mut last_error = String::new();
    for attempt in 0..=MAX_RETRIES {
        let response = match client
            .post(GROQ_ENDPOINT)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                return AiAdvisorResult {
                    suggestions: Vec::new(),
                    error: format!("Error de conexión con Groq: {e}"),
                };
            }
        };

        if response.status() == 429 {
            if attempt < MAX_RETRIES {
                // Extraer el tiempo de espera del body si es posible.
                let body = response.text().unwrap_or_default();
                let wait_ms = parse_retry_after(&body).unwrap_or(5000 + attempt as u64 * 3000);
                std::thread::sleep(std::time::Duration::from_millis(wait_ms));
                last_error = format!("Rate limit (reintento {}/{})", attempt + 1, MAX_RETRIES);
                continue;
            }
            let body = response.text().unwrap_or_default();
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: format!("Groq rate limit agotado tras {MAX_RETRIES} reintentos: {body}"),
            };
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: format!("Groq respondió HTTP {status}: {body}"),
            };
        }

        // Parsear la respuesta exitosa.
        let chat: ChatResponse = match response.json() {
            Ok(c) => c,
            Err(e) => {
                return AiAdvisorResult {
                    suggestions: Vec::new(),
                    error: format!("Error al parsear respuesta de Groq: {e}"),
                };
            }
        };

        // ... continue after loop
        last_error.clear(); // marcar éxito
                            // Extraer contenido y continuar con el parseo de sugerencias.
        return parse_response(chat);
    }

    // Si llegamos aquí, algo salió mal.
    AiAdvisorResult {
        suggestions: Vec::new(),
        error: if last_error.is_empty() {
            "Respuesta vacía del LLM".into()
        } else {
            last_error
        },
    }
}

/// Parsea la respuesta del chat y extrae sugerencias.
fn parse_response(chat: ChatResponse) -> AiAdvisorResult {
    let choice = match chat.choices.first() {
        Some(c) => c,
        None => {
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: "Groq devolvió una respuesta vacía.".into(),
            };
        }
    };

    // Intentar extraer contenido de `content` primero.
    let content_str = choice.message.content.trim();

    if content_str.is_empty() {
        // content vacío: puede ser rate-limit de reasoning o modelo no respondió.
        return AiAdvisorResult {
            suggestions: Vec::new(),
            error: format!(
                "El LLM devolvió contenido vacío (finish_reason: {}). \
                 Esto puede ocurrir si el modelo se quedó sin tokens de reasoning. \
                 Intenta de nuevo.",
                choice.finish_reason.as_deref().unwrap_or("desconocido")
            ),
        };
    }

    // El LLM podría envolver el JSON en markdown fences; limpiar.
    let json_str = strip_markdown_fences(content_str);

    let raw: Vec<RawSuggestion> = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            // Log del contenido crudo para debug.
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: format!(
                    "Error al parsear sugerencias del LLM: {e}\nContenido recibido (primeros 200 chars): {:?}",
                    &json_str[..json_str.len().min(200)]
                ),
            };
        }
    };

    let suggestions = raw
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .filter_map(convert_raw)
        .collect();

    AiAdvisorResult {
        suggestions,
        error: String::new(),
    }
}

/// Extrae el tiempo de reintento (ms) de un body de error 429.
/// Busca el patrón "Please try again in X.XXXs" en la respuesta de Groq.
fn parse_retry_after(body: &str) -> Option<u64> {
    // Ejemplo: "...Please try again in 18.2475s..."
    if let Some(start) = body.find("try again in ") {
        let rest = &body[start + 13..];
        if let Some(end) = rest.find('s') {
            if let Ok(secs) = rest[..end].parse::<f64>() {
                return Some((secs * 1000.0) as u64);
            }
        }
    }
    None
}

/// Convierte una sugerencia raw del LLM en una [`Suggestion`] del protocolo.
fn convert_raw(raw: RawSuggestion) -> Option<Suggestion> {
    let action = match raw.action {
        RawAction::None => SuggestionAction::None,
        RawAction::ApplyPreset { ref preset } => {
            let p = match preset.as_str() {
                "dry" => voxlfa_core::protocol::PresetId::Dry,
                "vozLimpia" => voxlfa_core::protocol::PresetId::VozLimpia,
                "radio" => voxlfa_core::protocol::PresetId::Radio,
                "warm" => voxlfa_core::protocol::PresetId::Warm,
                _ => return None,
            };
            SuggestionAction::ApplyPreset { preset: p }
        }
        RawAction::SetEqBand {
            band_index,
            gain_db,
        } => SuggestionAction::SetEqBand {
            band_index,
            gain_db: gain_db.clamp(-18.0, 18.0),
        },
        RawAction::SetDenoise { mix } => SuggestionAction::SetDenoise {
            mix: mix.clamp(0.0, 1.0),
        },
        RawAction::SetFeedback { threshold_db, q } => SuggestionAction::SetFeedback {
            threshold_db: threshold_db.clamp(-60.0, -10.0),
            q: q.clamp(2.0, 30.0),
        },
        RawAction::SetPitchCorrection { strength, mix } => SuggestionAction::SetPitchCorrection {
            strength: strength.clamp(0.0, 1.0),
            mix: mix.clamp(0.0, 1.0),
        },
        RawAction::SetNoiseGate {
            threshold_db,
            range_db,
        } => SuggestionAction::SetNoiseGate {
            threshold_db: threshold_db.clamp(-60.0, -10.0),
            range_db: range_db.clamp(0.0, 60.0),
        },
    };

    Some(Suggestion {
        id: raw.id,
        kind: SuggestionKind::AiAdvisor,
        severity: raw.severity.clamp(0.0, 1.0),
        message: raw.message,
        action,
    })
}

/// Elimina posibles fences markdown (` ```json ... ``` `) y tags `<think>`
/// alrededor del JSON. Qwen3 agrega razonamiento thinking antes del JSON.
fn strip_markdown_fences(s: &str) -> &str {
    let mut result = s.trim();

    // Qwen3 agrega <think>...</think> antes del JSON real.
    if let Some(end) = result.rfind("</think>") {
        result = result[end + 8..].trim();
    } else if let Some(end) = result.rfind("</think>") {
        result = result[end + 9..].trim();
    }

    if result.starts_with("```") {
        let after_first = result
            .find('\n')
            .map(|i| &result[i + 1..])
            .unwrap_or(result);
        if let Some(end) = after_first.rfind("```") {
            return after_first[..end].trim();
        }
        return after_first.trim();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_markdown_fences_removes_backticks() {
        let input = "```json\n[{\"id\":1}]\n```";
        assert_eq!(strip_markdown_fences(input), "[{\"id\":1}]");
    }

    #[test]
    fn strip_markdown_fences_passes_plain_json() {
        let input = "[{\"id\":1}]";
        assert_eq!(strip_markdown_fences(input), "[{\"id\":1}]");
    }

    #[test]
    fn strip_markdown_fences_removes_think_tag() {
        let input = "<think>\nAnalizando métricas...\n</think>\n[{\"id\":1}]";
        assert_eq!(strip_markdown_fences(input), "[{\"id\":1}]");
    }

    #[test]
    fn strip_markdown_fences_removes_think_tag_short() {
        let input = "<think>\nrazonamiento\n</think>\n[{\"id\":1}]";
        assert_eq!(strip_markdown_fences(input), "[{\"id\":1}]");
    }

    #[test]
    fn strip_markdown_fences_handles_think_with_fences() {
        let input = "<think>\nalgo\n</think>\n```json\n[{\"id\":1}]\n```";
        assert_eq!(strip_markdown_fences(input), "[{\"id\":1}]");
    }

    #[test]
    fn convert_raw_none_action_works() {
        let raw = RawSuggestion {
            id: 1,
            severity: 0.5,
            message: "Test".into(),
            action: RawAction::None,
        };
        let s = convert_raw(raw).unwrap();
        assert_eq!(s.action, SuggestionAction::None);
        assert_eq!(s.kind, SuggestionKind::AiAdvisor);
    }

    #[test]
    fn convert_raw_invalid_preset_returns_none() {
        let raw = RawSuggestion {
            id: 1,
            severity: 0.5,
            message: "Test".into(),
            action: RawAction::ApplyPreset {
                preset: "invalid".into(),
            },
        };
        assert!(convert_raw(raw).is_none());
    }

    #[test]
    fn convert_raw_set_eq_band_clamps_values() {
        let raw = RawSuggestion {
            id: 2,
            severity: 0.7,
            message: "Boost highs".into(),
            action: RawAction::SetEqBand {
                band_index: 5,
                gain_db: 25.0,
            },
        };
        let s = convert_raw(raw).unwrap();
        match s.action {
            SuggestionAction::SetEqBand { gain_db, .. } => assert_eq!(gain_db, 18.0),
            _ => panic!("wrong action"),
        }
    }

    #[test]
    fn request_suggestions_empty_key_returns_error() {
        let metrics = VoiceMetrics {
            rms_db: -20.0,
            peak_db: -6.0,
            dynamic_range_db: 14.0,
            crest_db: 14.0,
            brightness: 0.5,
            resonance_score: 0.3,
            fatigue_score: 0.2,
            window_ms: 2000,
        };
        let analysis = AnalysisSample {
            metrics,
            suggestions: Vec::new(),
            captured_at_ms: 0,
        };
        let result = request_suggestions("", &analysis, None);
        assert!(!result.error.is_empty());
        assert!(result.suggestions.is_empty());
    }
}
