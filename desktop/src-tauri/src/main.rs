//! Punto de entrada de la app de escritorio.

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
