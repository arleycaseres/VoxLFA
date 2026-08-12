//! Script de build del crate desktop.
//!
//! `tauri_build::build()` genera el código de contexto de Tauri (iconos y
//! configuración embebidos). Solo se necesita cuando el feature `webview`
//! está activo; con `--no-default-features` se compila el shell de backend
//! puro (sin UI), útil para pruebas en entornos sin webkit.

fn main() {
    #[cfg(feature = "webview")]
    tauri_build::build();
}
