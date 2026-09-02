//! Punto de entrada de la app de escritorio.

// Asignador global del binario. VoxLFA reconstruye módulos con buffers de
// tamaño variable al cambiar de preset (Harmonizer/Reverb/Delay); mimalloc
// reduce la fragmentación por liberación repetida que glibc acumula y que se
// ve como crecimiento de RSS/swap. Sigue siendo `Send + Sync` y solo actúa
// sobre este binario.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "webview")]
fn main() {
    voxlfa_desktop_lib::run();
}

#[cfg(not(feature = "webview"))]
fn main() {
    eprintln!(
        "VoxLFA se compiló sin el feature `webview`: no hay interfaz gráfica. \
         Use `cargo build --features webview` en un sistema con webkit2gtk."
    );
    std::process::exit(1);
}
