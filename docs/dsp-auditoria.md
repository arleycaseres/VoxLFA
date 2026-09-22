# Auditoría del DSP — sonido correcto por defecto

> Fuente de las «Reglas de oro del DSP» de `AGENTS.md`. Léelo junto a las otras
> guías: `arquitectura.md` describe qué hace cada módulo, este documento define
> con qué criterio de *calidad* se audita cada módulo DSP.

## Por qué existe este documento

Los módulos DSP de VoxLFA se desarrollaron **por módulos (fases)**. Cada módulo
compila, pasa sus tests unitarios y suena "bien". Pero compilar y pasar tests no
garantiza que el procesamiento producido sea *musicalmente correcto*: hay una
clase de bugs en los que el código "funciona" pero la señal está mal procesada,
no aísla la banda correcta, no se recupera después de un pico, o puede volverse
NaN/OOM con parámetros reales (especialmente los que llegan por la red).

Esta auditoría recorre la cadena DSP con **criterio de ingeniero de audio**, no
solo de programador, y registra cada defecto junto con el fix y el test de
regresión que lo demuestra.

## Metodología

Para cada módulo `dsp/` se audita, en este orden:

1. **El requisito sonoro**: ¿qué debe hacer? (¿extraer una banda? ¿ganancia?
   ¿recuperación temporal?). Determinar qué sería "oír correcto".
2. **El diseño de coeficientes**: qué `BiquadKind`/fórmula usa y con qué
   valores por defecto. Detectar **identidades** (ganancia 0 en filtro que debe
   extraer) y **NaN/degeneración** con params extremos o no finitos.
3. **El hot path por muestra**: cuántas veces por muestra avanza el estado de
   cada filtro/detector. Detectar dobles procesamientos y detectores sin release.
4. **Seguridad ante la red**: qué parámetros públicos llegan al tamaño de un
   buffer (`ms_to_samples`, `filter_len`) o a fórmulas sin clamp.
5. **Test de regresión**: escribir un test unitario que **falle con el bug** y
   pase con el fix, describiendo un comportamiento audible (ver estándares
   abajo).

## Lotes

### Lote 1 — Robustez heredada (A0.x) ✅

Hallazgos de la primera pasada, arrastrados de entregas anteriores:

- **A0.1 — Harmonizer con intervalos extremos**: `powf(2, interval/12)` podía
  generar ventanas/retardos gigantes. Fix en `dsp/harmonizer.rs`: clamp de
  intervalos/gain y `is_finite()` + test
  `extreme_intervals_do_not_hang_or_panic`.
- **A0.2 — Denoise `enabled` no propagado**: conmutar denoise no cambiaba el
  `mix`/passthrough real. Fix en `dsp/chain.rs` + `denoise_mix()`: propagar el
  estado `enabled` y el mix (spawn/SetDenoise/ApplyPreset).
- **A0.3 — `DenoiseHandle` sin Drop**: leak del hilo de denoise al reconfigurar
  la cadena. Fix: `impl Drop` (stop + join) y se quita el
  `allow(dead_code)` de `AtomicF32::store`.
- **A0.4 — Cola fantasma del denoise**: el denoise dejaba una "cola" de ruido
  fuera de orden por el retardo de sus frames. Fix en `dsp/chain.rs`:
  `copy_from_slice` de la cola pre-denoise para no dejar silencio/eco fantasma.
- **A0.5 — Falso positivo (sin cambios)**: se investigó el parsing de tags
  `<thinking>` en `desktop/src-tauri/src/llm.rs`; el "bug" era un artefacto de
  renderizado de la herramienta. Sin cambios de código. **Lección**: verificar
  el contenido literal de secuencias `<...>` con `od -a`/hexdump/Python antes de
  tocar código (los renderizadores de terminal las ocultan).

### Prioridades y estado

La auditoría se divide en lotes; los lotes terminados quedan en verde con su
batería de verificación (fmt, clippy `-D warnings`, tests) corrida *una vez por
lote*:

| Lote | Contenido | Estado |
| --- | --- | --- |
| Lote 1 | Bugs de robustez arrastrados de entregas anteriores | ✅ |
| Lote 2 | Bandas identidad, doble procesado, limiter sin release, clamps/OOM | ✅ |
| Lote 3 | EQ/Sculpt: verificación de bandas con ganancia real | ✅ |
| Lote 4 | Feedback, denoise, pitch/vocal isolation: revisión de estabilidad | pendiente |

Cada lote nuevo debe **recorrer todos los módulos** con la metodología de arriba
y, al cerrarse, ejecutar la batería completa de `AGENTS.md`.

## Hallazgos y fixes (referencia)

### Bandas identidad (`Peaking` con gain 0)

**Síntoma**: de-esser que "reduce todo", analizador de bandas con ratios ~1.

- `core/src/dsp/deesser.rs`, `core/src/dsp/boomsuppressor.rs`: usaban
  `BiquadKind::Peaking` con `gain_db: 0.0` → biquad identidad `H(z) ≡ 1` →
  atenuaban TODO el espectro. Fix: `BiquadKind::BandPass`.
- `core/src/analysis/bands.rs`: `lowmid`/`mid` eran `Peaking` a 0 dB → los
  ratios `lowmid_ratio`/`mid_ratio` no aislaban. Fix: `BandPass`.
- Tests: `off_band_signal_is_not_affected` (de-esser y boomsuppressor),
  `lowmid_band_isolates_boom_frequency`, `mid_band_isolates_presence_frequency`.

**Regla derivada**: un filtro que debe *extraer* una banda NUNCA es un
`Peaking` con gain 0. BandPass/HighPass/LowPass.

### Doble procesado por muestra (`dynamic_eq.rs`)

**Síntoma**: filtro con estado corrompido → sonido incorrecto e impredecible.

- `DynamicEqBand::process_sample()` procesaba el bandpass internamente, y el
  loop del caller lo volvía a procesar con `band.bandpass.process(dry)` → el
  estado del biquad avanzaba DOS veces por muestra. Fix: `process_sample`
  devuelve `(band_original, wet)` y el caller reconstruye
  `input + wet - band_original` sin reprocesar.

**Regla derivada**: cada biquad se avanza exactamente una vez por muestra.

### Limiter sin release (`limiter.rs`)

**Síntoma**: tras un pico transitorio el limitador queda atenuando para siempre.

- El pico detector solo subía (`if x > peak { peak = x }`). Fix: decaimiento
  `peak *= (1.0 - release_coef)` antes de la comparación + test
  `gain_recovers_after_transient_peak`.

**Regla derivada**: todo detector de pico/envolvente tiene release.

### Clamps y estabilidad numérica (`biquad.rs`, `saturator.rs`)

**Síntoma**: NaN/inf por params extremos o no finitos.

- `biquad.rs` `design()`: NaN en `q`/`gain_db` o `sample_rate == 0` producían
  coeficientes NaN (contaminan la salida para siempre). Fix: passthrough ante
  valores no finitos/0, `q` clamp `[0.01, 100]`, `gain_db` clamp `±24 dB`.
- `saturator.rs` `tube_saturate`: `(e^x − e^−x)/(e^x + e^−x)` desborda a
  NaN/inf con `drive` grande. Fix: `x.tanh()` (matemáticamente idéntico,
  estable). `tape_saturate`: clamp de `x`.
- Tests: `non_finite_params_degrade_to_passthrough`,
  `extreme_q_and_gain_are_clamped_not_nan`, `extreme_drive_never_produces_nan`.

**Regla derivada**: `tanh` en vez de `exp`; `is_finite()` + clamp antes de
fórmulas; degradar a passthrough, nunca propagar NaN.

### OOM y NaN por params de red (`delay.rs`, `reverb.rs`)

**Síntoma**: tiempo de delay/reverb gigante (p. ej. `1e9 ms`) asignaría un
buffer de cientos de GB → OOM; NaN → 0 con buffer mínimo o comportamiento raro.

- `ms_to_samples()` (delay y reverb): sin cota superior ni guard de NaN. Fix:
  NaN → 0, clamp a un máximo (5 s delay, 2 s reverb), y `Delay::from_params`
  acota `time_ms` a 2000 ms / `pre_delay_ms` a 1000 ms.
- Tests: `huge_times_are_capped_not_oom`, `nan_times_degrade_to_zero_delay`,
  `huge_pre_delay_is_capped_not_oom`, `nan_param_does_not_panic`.

**Regla derivada**: cualquier parámetro de red que dimensione memoria se acota
antes. El caller (protocolo) manda valores arbitrarios; el DSP es la última
línea de defensa.

### Guild de ganancia real (`eq.rs`, `sculpt.rs`)

**Síntoma**: un `update_params` que guarda params pero no rediseña los biquads
deja filtros con coeficientes del tono anterior (estado inconsistente).

- `eq.rs`: verificado que las bandas a 0 dB son identidad exacta (por diseño del
  cookbook RBJ con `A = 1`), así que una banda neutral no altera el resto del
  espectro. Tests: `cuts_a_single_band` (asimetría de boost, −12 dB atenúa de
  verdad) y `zero_gain_band_is_identity`.
- `sculpt.rs`: `update_params()` solo asignaba `self.params = params` sin
  reconstruir `bass`/`presence`/`air` → tras un update el `process` decidía el
  passthrough con el tono nuevo pero filtraba con los coeficientes del tono
  viejo (en la cadena real se reconstruye vía `SetLinkSculpt`, pero la API
  pública quedaba inconsistente). Fix: `update_params` re-diseña todos los
  filtros. Test: `update_params_rebuilds_filters` (falla con el bug: construye
  neutro y espera que update a brillante filtre).

**Regla derivada**: si `process` toma decisiones según `self.params`, cada
`update_params` debe mantener los coeficientes coherentes con esos params;
nunca dejar un procesador que "dice" una configuración y "suena" con otra.

## Estándares de los tests de regresión DSP

El objetivo de cada fix "correcto por defecto" es que su test **demuestre el
comportamiento audible** y falle con el bug original:

| Comportamiento audible | Forma del test |
| --- | --- |
| Seno dentro de banda se atenúa | RMS de salida < RMS de entrada (p. ej. `* 0.7`) |
| Seno fuera de banda intacto | `|rms_out − rms_in| < rms_in * 0.2` |
| El análisis aísla bandas | seno 300 Hz → `lowmid_ratio > mid_ratio` (β y viceversa) |
| El limitador se recupera | tras un pico único, la cola de salida vuelve a ~1 de ganancia |
| Sin NaN/inf con params extremos | `out.iter().all(|v| v.is_finite())` |
| Sin OOM con params de red | `buffer.len() < ms_to_samples(<máx>, sr) + 1` |

Usar `rms()` sobre la segunda mitad del buffer (dejar que el filtro se
estabilice) y vistas de 8192 muestras para señales sinusoidales con potencial de
transitorios de filtro.

## Cómo continuar

- El Lote 4 (Feedback, denoise, pitch/vocal isolation: revisión de estabilidad)
  está pendiente. Al abrirlo, cargar este documento y la metodología de arriba,
  recorrer los módulos con criterio de audio y cerrar con la batería de
  verificación completa de `AGENTS.md`.
- Después de cada fix, **releer el diff completo** buscando reintroducciones de
  estos patrones (identidades, dobles procesados, detectores sin release, clamps
  olvidados).