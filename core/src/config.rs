//! Persistencia de configuración y perfiles por dispositivo (Fase 3).
//!
//! `AppConfig` es el esquema del archivo `config.json` del usuario (ubicado en
//! `$XDG_CONFIG_HOME/voxlfa/config.json` o `~/.config/voxlfa/config.json` en
//! Linux). No forma parte del contrato de red con el móvil: es un esquema de
//! persistencia local del escritorio.
//!
//! Los perfiles se indexan por el **nombre del dispositivo de entrada** elegido
//! (o la clave `"default"` cuando se usa el predeterminado del sistema). Cada
//! perfil recuerda el preset, el ajuste fino del EQ y los bypasses de ese
//! dispositivo para reaplicarlos al arrancar.
//!
//! La escritura es tolerante a fallos: si el archivo no existe o está corrupto,
//! se parte de una configuración vacía; si no se puede guardar, se ignora el
//! error (la sesión sigue funcionando).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::protocol::{
    DelayParams, DenoiseParams, EqBand, FeedbackSuppressorParams, NoiseGateParams,
    PitchCorrectionParams, PresetId, ReverbParams,
};
use crate::Result;

/// Clave de perfil cuando se arranca con el dispositivo predeterminado.
pub const DEFAULT_DEVICE_KEY: &str = "default";

/// Configuración persistida de la aplicación.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Último host de audio elegido (`None` = predeterminado del sistema).
    /// Se usa para precargar el selector de hosts de la cabina.
    #[serde(default)]
    pub default_host: Option<String>,
    /// Último dispositivo de entrada elegido (`None` = predeterminado del
    /// sistema). Se usa para precargar el selector de la cabina.
    #[serde(default)]
    pub default_input: Option<String>,
    /// Último dispositivo de salida elegido (`None` = predeterminado).
    #[serde(default)]
    pub default_output: Option<String>,
    /// Último tamaño de buffer elegido (`None` = auto por heurística).
    #[serde(default)]
    pub buffer_size: Option<usize>,
    /// Perfiles guardados, uno por dispositivo de entrada.
    #[serde(default)]
    pub profiles: Vec<DeviceProfile>,
    /// Consentimiento de telemetría: `Some(true)` = activada,
    /// `Some(false)` = desactivada, `None` = sin decidir (mostrar diálogo).
    #[serde(default)]
    pub telemetry_enabled: Option<bool>,
}

/// Perfil recordado para un dispositivo de entrada concreto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProfile {
    /// Clave del perfil: nombre del dispositivo de entrada o `"default"`.
    pub device_key: String,
    /// Preset recordado para este dispositivo.
    #[serde(default)]
    pub preset: PresetId,
    /// Ajuste fino del EQ (bandas actuales) para el preset de este perfil.
    #[serde(default)]
    pub eq_bands: Vec<EqBand>,
    /// Parámetros de la puerta de ruido si el preset de este perfil la tiene y
    /// se ajustaron en vivo; `None` = usar los del preset.
    #[serde(default)]
    pub gate_params: Option<NoiseGateParams>,
    /// Parámetros de denoise si el preset de este perfil lo tiene y se
    /// ajustaron en vivo; `None` = usar los del preset.
    #[serde(default)]
    pub denoise_params: Option<DenoiseParams>,
    /// Parámetros de feedback suppressor si el preset de este perfil lo tiene
    /// y se ajustaron en vivo; `None` = usar los del preset.
    #[serde(default)]
    pub feedback_params: Option<FeedbackSuppressorParams>,
    /// Parámetros de corrección de tono si el preset de este perfil lo tiene
    /// y se ajustaron en vivo; `None` = usar los del preset.
    #[serde(default)]
    pub pitch_correction_params: Option<PitchCorrectionParams>,
    /// Parámetros de delay si el preset de este perfil lo tiene y se ajustaron
    /// en vivo; `None` = usar los del preset.
    #[serde(default)]
    pub delay_params: Option<DelayParams>,
    /// Parámetros de reverb si el preset de este perfil lo tiene y se ajustaron
    /// en vivo; `None` = usar los del preset.
    #[serde(default)]
    pub reverb_params: Option<ReverbParams>,
    /// `true` si el bypass global estaba activo al guardar.
    #[serde(default)]
    pub global_bypass: bool,
    /// Bypass por módulo que estaba activo al guardar.
    #[serde(default)]
    pub link_bypass: HashMap<String, bool>,
}

impl AppConfig {
    /// Devuelve el perfil de un dispositivo de entrada, si existe.
    pub fn profile(&self, device_key: &str) -> Option<&DeviceProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.device_key == device_key)
    }

    /// Devuelve (o crea) el perfil mutable de un dispositivo de entrada.
    pub fn profile_mut(&mut self, device_key: &str) -> &mut DeviceProfile {
        if let Some(position) = self
            .profiles
            .iter()
            .position(|profile| profile.device_key == device_key)
        {
            return &mut self.profiles[position];
        }
        self.profiles.push(DeviceProfile {
            device_key: device_key.to_string(),
            preset: PresetId::default(),
            eq_bands: Vec::new(),
            gate_params: None,
            denoise_params: None,
            feedback_params: None,
            pitch_correction_params: None,
            delay_params: None,
            reverb_params: None,
            global_bypass: false,
            link_bypass: HashMap::new(),
        });
        let last = self.profiles.len() - 1;
        &mut self.profiles[last]
    }
}

/// Ruta al archivo de configuración del usuario (solo Linux/Unix).
///
/// Respeta `$XDG_CONFIG_HOME` si está definido; si no, cae a
/// `~/.config/voxlfa/config.json`. Devuelve `None` si no hay HOME
/// configurado (entornos mínimos); en ese caso no hay persistencia.
pub fn default_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("voxlfa").join("config.json"))
}

/// Carga/guarda [`AppConfig`] en un archivo JSON, tolerante a fallos.
#[derive(Debug)]
pub struct ConfigStore {
    /// Ruta del archivo; vacía si solo se usa en memoria (sin persistencia).
    path: PathBuf,
    /// Configuración en memoria.
    config: AppConfig,
}

impl ConfigStore {
    /// Carga la configuración desde la ruta indicada.
    ///
    /// Si el archivo no existe o no es JSON válido, se devuelve una
    /// configuración vacía (la escritura posterior la sobrescribe).
    pub fn load(path: &Path) -> Self {
        let config = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        Self {
            path: path.to_path_buf(),
            config,
        }
    }

    /// Store solo en memoria: `save()` no hace nada.
    pub fn memory() -> Self {
        Self {
            path: PathBuf::new(),
            config: AppConfig::default(),
        }
    }

    /// Guarda la configuración en su archivo (creando el directorio padre).
    ///
    /// Es una operación best-effort: los errores se propagan al llamador,
    /// que decide si interrumpen la operación en curso.
    pub fn save(&self) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }

    /// Configuración en memoria (solo lectura).
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Configuración en memoria (mutable).
    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{EqBandKind, PresetId};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("voxlfa-config-test-{name}-{}", std::process::id()))
    }

    fn band(kind: EqBandKind, freq_hz: f32, gain_db: f32) -> EqBand {
        EqBand {
            kind,
            freq_hz,
            gain_db,
            q: 1.0,
        }
    }

    #[test]
    fn missing_file_loads_empty_config() {
        let store = ConfigStore::load(&temp_path("missing"));
        assert_eq!(store.config().profiles.len(), 0);
        assert_eq!(store.config().default_input, None);
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = temp_path("roundtrip");
        let mut config = AppConfig {
            default_input: Some("Micrófono (USB Audio)".into()),
            default_output: Some("Altavoces (USB Audio)".into()),
            buffer_size: Some(128),
            ..AppConfig::default()
        };
        {
            let profile = config.profile_mut("Micrófono (USB Audio)");
            profile.preset = PresetId::VozLimpia;
            profile.eq_bands = vec![band(EqBandKind::Peaking, 3000.0, 3.5)];
            profile.gate_params = Some(NoiseGateParams {
                threshold_db: -52.0,
                attack_ms: 2.0,
                release_ms: 90.0,
                hold_ms: 20.0,
                range_db: 40.0,
            });
            profile.global_bypass = false;
            profile.link_bypass.insert("reverb".into(), true);
        }
        let store = ConfigStore {
            path: path.clone(),
            config,
        };
        store.save().expect("guarda la configuración");

        let loaded = ConfigStore::load(&path);
        assert_eq!(
            loaded.config().default_input.as_deref(),
            Some("Micrófono (USB Audio)")
        );
        assert_eq!(loaded.config().buffer_size, Some(128));
        let profile = loaded
            .config()
            .profile("Micrófono (USB Audio)")
            .expect("perfil cargado");
        assert_eq!(profile.preset, PresetId::VozLimpia);
        assert_eq!(profile.eq_bands[0].gain_db, 3.5);
        assert_eq!(profile.gate_params.map(|p| p.threshold_db), Some(-52.0));
        assert_eq!(profile.link_bypass.get("reverb"), Some(&true));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupted_file_falls_back_to_empty() {
        let path = temp_path("corrupt");
        std::fs::write(&path, "no soy json").expect("escribe basura");
        let store = ConfigStore::load(&path);
        assert_eq!(store.config().profiles.len(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn profile_mut_creates_then_returns_existing() {
        let mut config = AppConfig::default();
        let first = config.profile_mut("A");
        first.preset = PresetId::Radio;
        // El segundo acceso debe devolver el mismo perfil (no duplicarlo).
        let second = config.profile_mut("A");
        assert_eq!(second.preset, PresetId::Radio);
        assert_eq!(config.profiles.len(), 1);
    }

    #[test]
    fn json_uses_camel_case_names() {
        let mut config = AppConfig {
            default_input: Some("mic".into()),
            buffer_size: Some(256),
            ..AppConfig::default()
        };
        config.profile_mut("mic").preset = PresetId::Warm;
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"defaultInput\""));
        assert!(json.contains("\"bufferSize\""));
        assert!(json.contains("\"deviceKey\":\"mic\""));
        assert!(json.contains("\"preset\":\"warm\""));
        assert!(json.contains("\"eqBands\":[]"));
        assert!(json.contains("\"gateParams\":null"));
        assert!(json.contains("\"denoiseParams\":null"));
        assert!(json.contains("\"globalBypass\":false"));
        assert!(json.contains("\"linkBypass\":{}"));
    }

    #[test]
    fn default_config_path_prefers_xdg_config_home() {
        let old = std::env::var_os("XDG_CONFIG_HOME");
        let home = std::env::var_os("HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/voxlfa-xdg");
        let path = default_config_path().expect("hay XDG_CONFIG_HOME");
        assert_eq!(path, PathBuf::from("/tmp/voxlfa-xdg/voxlfa/config.json"));

        if home.is_some() {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::set_var("HOME", "/tmp/voxlfa-home");
            let path = default_config_path().expect("hay HOME");
            assert_eq!(
                path,
                PathBuf::from("/tmp/voxlfa-home/.config/voxlfa/config.json")
            );
        }

        if let Some(value) = old {
            std::env::set_var("XDG_CONFIG_HOME", value);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        if let Some(value) = home {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
