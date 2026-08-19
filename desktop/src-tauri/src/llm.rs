// Asesor de configuración con IA (Groq): envía métricas de voz a un LLM y
// devuelve sugerencias concretas de ajuste de la cadena DSP.
//
// El módulo es independiente de Tauri: recibe métricas, construye el prompt,
// llama a la API de Groq (OpenAI-compatible) y parsea la respuesta JSON
// en [`Suggestion`]. Los comandos Tauri lo orquestan.

use voxlfa_core::protocol::{
    AnalysisSample, Suggestion, SuggestionAction, SuggestionKind, VoiceMetrics,
};

/// Endpoint de la API de Groq (OpenAI-compatible).
const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Modelo por defecto en Groq.
const GROQ_MODEL: &str = "llama-3.3-70b-versatile";

/// Timeout de la petición LLM en segundos.
const LLM_TIMEOUT_SECS: u64 = 15;

/// Número máximo de sugerencias que esperamos del LLM.
const MAX_SUGGESTIONS: usize = 5;

/// Construye el prompt del sistema que describe el rol del asistente.
fn system_prompt() -> String {
    "Eres el asistente técnico de VoxLFA, un procesador vocal en vivo. \
     Analizas métricas de audio en tiempo real y recomiendas ajustes concretos \
     de la cadena DSP.\n\n\
     Reglas:\n\
     - Responde SOLO con JSON válido, sin texto adicional ni markdown.\n\
     - Devuelve un array JSON de 0 a 5 sugerencias.\n\
     - Cada sugerencia tiene: id (número entero), kind (\"aiAdvisor\"), \
     severity (0–1), message (explicación breve en español), \
     action (objeto con campo type).\n\
     - Tipos de acción válidos:\n\
       {\"type\":\"none\"}\n\
       {\"type\":\"applyPreset\",\"preset\":\"dry\"|\"vozLimpia\"|\"radio\"|\"warm\"}\n\
       {\"type\":\"setEqBand\",\"bandIndex\":0-6,\"gainDb\":-18..18}\n\
       {\"type\":\"setDenoise\",\"mix\":0..1}\n\
       {\"type\":\"setFeedback\",\"thresholdDb\":-60..-10,\"q\":2..30}\n\
       {\"type\":\"setPitchCorrection\",\"strength\":0..1,\"mix\":0..1}\n\
     - Si la voz está equilibrada, devuelve un array vacío []"
        .to_string()
}

/// Construye el prompt del usuario con las métricas actuales.
fn user_prompt(metrics: &VoiceMetrics) -> String {
    format!(
        "Métricas de voz actuales:\n\
         - RMS: {rms:.1} dBFS\n\
         - Pico: {peak:.1} dBFS\n\
         - Rango dinámico: {dr:.1} dB\n\
         - Cresta: {crest:.1} dB\n\
         - Brillo (0–1): {brightness:.3}\n\
         - Resonancia baja-media (0–1): {resonance:.3}\n\
         - Fatiga vocal (0–1): {fatigue:.3}\n\n\
         Analiza estas métricas y recomienda los ajustes más útiles.",
        rms = metrics.rms_db,
        peak = metrics.peak_db,
        dr = metrics.dynamic_range_db,
        crest = metrics.crest_db,
        brightness = metrics.brightness,
        resonance = metrics.resonance_score,
        fatigue = metrics.fatigue_score,
    )
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
}

#[derive(serde::Deserialize)]
struct ChatResponseMessage {
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
}

/// Resultado de una petición al asesor de IA.
#[derive(Debug, Clone)]
pub struct AiAdvisorResult {
    /// Sugerencias generadas por el LLM (vacío si la voz está equilibrada).
    pub suggestions: Vec<Suggestion>,
    /// Mensaje de error si la petición falló (vacío si fue exitosa).
    pub error: String,
}

/// Pide consejo al LLM con las métricas dadas.
///
/// `api_key` es la clave de la API de Groq. Si está vacía o la petición falla,
/// devuelve un [`AiAdvisorResult`] con `error` no vacío.
pub fn request_suggestions(api_key: &str, analysis: &AnalysisSample) -> AiAdvisorResult {
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
                content: user_prompt(&analysis.metrics),
            },
        ],
        temperature: 0.3,
        max_tokens: 1024,
    };

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

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return AiAdvisorResult {
            suggestions: Vec::new(),
            error: format!("Groq respondió HTTP {status}: {body}"),
        };
    }

    let chat: ChatResponse = match response.json() {
        Ok(c) => c,
        Err(e) => {
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: format!("Error al parsear respuesta de Groq: {e}"),
            };
        }
    };

    let content = match chat.choices.first() {
        Some(c) => c.message.content.trim(),
        None => {
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: "Groq devolvió una respuesta vacía.".into(),
            };
        }
    };

    // El LLM podría envolver el JSON en markdown fences; limpiar.
    let json_str = strip_markdown_fences(content);

    let raw: Vec<RawSuggestion> = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            return AiAdvisorResult {
                suggestions: Vec::new(),
                error: format!("Error al parsear sugerencias del LLM: {e}"),
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
    };

    Some(Suggestion {
        id: raw.id,
        kind: SuggestionKind::AiAdvisor,
        severity: raw.severity.clamp(0.0, 1.0),
        message: raw.message,
        action,
    })
}

/// Elimina posibles fences markdown (` ```json ... ``` `) alrededor del JSON.
fn strip_markdown_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.starts_with("```") {
        // Saltar la primera línea (```json o ```)
        let after_first = trimmed
            .find('\n')
            .map(|i| &trimmed[i + 1..])
            .unwrap_or(trimmed);
        // Quitar el fence de cierre.
        if let Some(end) = after_first.rfind("```") {
            return after_first[..end].trim();
        }
        return after_first.trim();
    }
    trimmed
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
        let result = request_suggestions("", &analysis);
        assert!(!result.error.is_empty());
        assert!(result.suggestions.is_empty());
    }
}
