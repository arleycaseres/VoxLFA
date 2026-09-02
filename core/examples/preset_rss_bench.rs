//! Benchmark de RSS: mide la huella de memoria al cambiar de preset en una
//! sesión con audio en vivo.
//!
//! `apply_preset` reconstruye la cadena DSP (Harmonizer/Reverb/Delay con
//! buffers de tamaño variable). Para simular la sesión real, entre cada cambio
//! de preset se procesan `BLOCKS_PER_SWITCH` bloques de audio (los buffers de
//! audio viven y se reorganizan durante la reconstrucción, que es donde la
//! fragmentación del heap se materializa).
//!
//! Reporta el RSS (Linux, `/proc/self/status`) antes y después para comparar
//! glibc vs mimalloc:
//!
//!   cargo run --example preset_rss_bench --release            # glibc
//!   cargo run --example preset_rss_bench --release \
//!     --features bench-mimalloc                               # mimalloc
//!
//! Como test de regresión (punto 4): si se define `VOXLFA_RSS_THRESHOLD_MB`,
//! el binario falla (código de salida 1) si el RSS crece más del umbral tras
//! los 200 cambios. Umbral recomendado: 1 MB (holgado por encima del ruido
//! observado, ~0.3-0.5 MB, y lo bastante bajo para detectar una reintroducción
//! del crecimiento de memoria). No se fija en el CI compartido por ser
//! sensible al ruido del runner:
//!
//!   VOXLFA_RSS_THRESHOLD_MB=1 cargo run --example preset_rss_bench --release

use voxlfa_core::dsp::chain::ChainProcessor;
use voxlfa_core::dsp::processor::ProcessingInfo;
use voxlfa_core::protocol::dsp::PresetId;

#[cfg(feature = "bench-mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Presets de la cabina, en el orden que alterna el benchmark.
const PRESETS: [PresetId; 5] = [
    PresetId::VozLimpia,
    PresetId::Radio,
    PresetId::Warm,
    PresetId::Monitor,
    PresetId::Foh,
];

/// Número de cambios de preset del benchmark (40 ciclos completos de 5).
const SWITCHES: usize = 200;

/// Bloques de audio procesados entre cada cambio de preset.
const BLOCKS_PER_SWITCH: usize = 200;

/// Muestreo y tamaño de buffer representativos del escritorio.
const SAMPLE_RATE: u32 = 48_000;
const MAX_FRAMES: usize = 512;

/// Lee el RSS (kB) del proceso actual desde `/proc/self/status` (Linux).
fn rss_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status")
        .expect("no se pudo leer /proc/self/status (¿no es Linux?)");
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn main() {
    let allocator = std::env::var("VOXLFA_ALLOCATOR").unwrap_or_else(|_| {
        if cfg!(feature = "bench-mimalloc") {
            "mimalloc".to_string()
        } else {
            "system (glibc)".to_string()
        }
    });
    println!("allocator_size = {allocator}");
    println!("presets = {PRESETS:?}");
    println!(
        "switches = {SWITCHES} ({} ciclos completos) · {BLOCKS_PER_SWITCH} bloques/switch",
        SWITCHES / PRESETS.len()
    );

    // Construir la cadena y buffers de audio persistentes (viven toda la
    // sesión simulada, como en la app real).
    let mut chain = ChainProcessor::new(PRESETS[0], SAMPLE_RATE, MAX_FRAMES);
    let input = vec![0.02f32; MAX_FRAMES];
    let mut scratch = vec![0.0f32; MAX_FRAMES];
    let mut output = vec![0.0f32; MAX_FRAMES];
    let info = ProcessingInfo {
        sample_rate: SAMPLE_RATE,
        frames: MAX_FRAMES,
    };

    // Calentar para excluir la asignación única inicial del baseline.
    for _ in 0..BLOCKS_PER_SWITCH {
        chain.process_pre_denoise(&input, &mut scratch, &info);
        chain.process_post_denoise(&scratch, &mut output, &info);
    }

    let rss_before = rss_kb();
    println!("rss_before_kb = {rss_before}");

    for i in 0..SWITCHES {
        let preset = PRESETS[i % PRESETS.len()];
        chain.apply_preset(preset);
        for _ in 0..BLOCKS_PER_SWITCH {
            chain.process_pre_denoise(&input, &mut scratch, &info);
            chain.process_post_denoise(&scratch, &mut output, &info);
        }
    }

    let rss_after = rss_kb();
    let delta_kb = rss_after.saturating_sub(rss_before);
    let delta_mb = delta_kb as f64 / 1024.0;
    println!("rss_after_kb  = {rss_after}");
    println!("rss_delta_kb  = {delta_kb}");
    println!("rss_delta_mb  = {delta_mb:.3}");

    // Umbral de regresión opcional (punto 4): falla si se supera.
    if let Ok(raw) = std::env::var("VOXLFA_RSS_THRESHOLD_MB") {
        let threshold_mb: f64 = raw
            .parse()
            .expect("VOXLFA_RSS_THRESHOLD_MB debe ser un número (MB)");
        if delta_mb > threshold_mb {
            eprintln!(
                "FALLO: el RSS creció {delta_mb:.3} MB tras {SWITCHES} cambios de preset, \
                 superando el umbral de {threshold_mb} MB."
            );
            std::process::exit(1);
        }
        println!("umbral de regresión {threshold_mb} MB: OK (creció {delta_mb:.3} MB)");
    }
}
