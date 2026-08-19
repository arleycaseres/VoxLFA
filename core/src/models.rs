//! Gestión de modelos ONNX para DeepFilterNet3.
//!
//! Los modelos se almacenan en un directorio de datos de la aplicación
//! (distinto del de configuración) y se descargan bajo demanda desde los
//! assets de una release de GitHub.
//!
//! Directorio por plataforma:
//! - Linux:   `$XDG_DATA_HOME/voxlfa/models/` o `~/.local/share/voxlfa/models/`
//! - macOS:   `~/Library/Application Support/voxlfa/models/`
//! - Windows: `%LOCALAPPDATA%\voxlfa\models\`

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Nombre de los archivos que componen el modelo DeepFilterNet3.
const MODEL_FILES: &[&str] = &["enc.onnx", "erb_dec.onnx", "df_dec.onnx", "config.ini"];

/// URL base de los assets de release de GitHub donde se descargan los modelos.
///
/// El nombre de release se sustituye dinámicamente con la versión del crate.
const GITHUB_RELEASE_BASE: &str = "https://github.com/arleycaseres/VoxLFA/releases/download";

/// Estado de los modelos ONNX en el disco local.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    /// `true` si todos los archivos del modelo están presentes.
    pub available: bool,
    /// Directorio donde se almacenan los modelos.
    pub model_dir: String,
    /// Archivos que faltan (vacío si `available` es `true`).
    pub missing: Vec<String>,
}

impl ModelStatus {
    /// Comprueba el estado de los modelos en el directorio indicado.
    pub fn check(model_dir: &Path) -> Self {
        let missing: Vec<String> = MODEL_FILES
            .iter()
            .filter(|name| !model_dir.join(name).exists())
            .map(|name| name.to_string())
            .collect();
        Self {
            available: missing.is_empty(),
            model_dir: model_dir.display().to_string(),
            missing,
        }
    }
}

/// Devuelve el directorio de modelos para la plataforma actual.
///
/// En Linux respeta `$XDG_DATA_HOME`; en las demás plataformas usa el
/// estándar de la plataforma. Si no se puede determinar el HOME, devuelve
/// `None` (no hay persistencia posible).
pub fn models_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })?;
        Some(base.join("voxlfa").join("models"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("voxlfa")
                .join("models"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var_os("LOCALAPPDATA")?;
        Some(PathBuf::from(local).join("voxlfa").join("models"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Descarga un archivo desde una URL y lo escribe en `dest`.
///
/// Devuelve el número de bytes escritos. Usa HTTP simple sin dependencias
/// externas:resuelve la URL, descarga el contenido y lo guarda.
#[cfg(feature = "onnx")]
pub fn download_file(url: &str, dest: &Path) -> crate::Result<u64> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Descargar con ureq (disponible vía feature onnx).
    let response = ureq::get(url)
        .call()
        .map_err(|e| crate::Error::audio(format!("HTTP request failed: {e}")))?;

    let mut body = response.into_body();
    let mut reader = body.as_reader();
    let mut file = std::fs::File::create(dest)?;
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)
            .map_err(|e| crate::Error::audio(format!("read error: {e}")))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        total += n as u64;
    }
    Ok(total)
}

/// Descarga todos los modelos ONNX desde los assets de una release de GitHub.
///
/// `version` es la etiqueta de release (p. ej. `"v0.1.0"`).
/// `progress_fn` se llama con `(archivo_actual, total_archivos)` para reportar
/// progreso. Devuelve el directorio donde se guardaron los modelos.
#[cfg(feature = "onnx")]
pub fn download_models<F>(version: &str, progress_fn: F) -> crate::Result<PathBuf>
where
    F: Fn(usize, usize),
{
    let dir = models_dir().ok_or_else(|| {
        crate::Error::audio("cannot determine models directory for this platform")
    })?;
    std::fs::create_dir_all(&dir)?;

    for (i, name) in MODEL_FILES.iter().enumerate() {
        progress_fn(i, MODEL_FILES.len());
        let url = format!("{}/{name}", base_download_url(version));
        let dest = dir.join(name);
        if dest.exists() {
            log::info!("model {name} already exists, skipping download");
            continue;
        }
        log::info!("downloading {name} from {url}");
        download_file(&url, &dest)?;
    }
    progress_fn(MODEL_FILES.len(), MODEL_FILES.len());
    Ok(dir)
}

/// URL base de descarga para una versión concreta.
pub fn base_download_url(version: &str) -> String {
    format!("{GITHUB_RELEASE_BASE}/{version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_status_reports_missing_files() {
        let dir = PathBuf::from("/nonexistent/path");
        let status = ModelStatus::check(&dir);
        assert!(!status.available);
        assert_eq!(status.missing.len(), MODEL_FILES.len());
    }

    #[test]
    fn model_status_available_when_all_files_present() {
        let dir = std::env::temp_dir().join(format!("voxlfa-models-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in MODEL_FILES {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let status = ModelStatus::check(&dir);
        assert!(status.available);
        assert!(status.missing.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn model_status_reports_partial_files() {
        let dir =
            std::env::temp_dir().join(format!("voxlfa-models-partial-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("enc.onnx"), b"").unwrap();
        let status = ModelStatus::check(&dir);
        assert!(!status.available);
        assert!(status.missing.contains(&"erb_dec.onnx".to_string()));
        assert!(status.missing.contains(&"df_dec.onnx".to_string()));
        assert!(status.missing.contains(&"config.ini".to_string()));
        assert!(!status.missing.contains(&"enc.onnx".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base_download_url_format() {
        let url = base_download_url("v0.1.0");
        assert!(url.ends_with("/v0.1.0"));
        assert!(url.contains("github.com"));
        assert!(url.contains("releases"));
    }
}
