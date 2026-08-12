# Arquitectura de VoxLFA

VoxLFA es un **procesador vocal en tiempo real** con IA. La Fase 0 entregó la
base (motor de audio de baja latencia, cabina de escritorio y monitor móvil
remoto); la **Fase 1** añade el DSP real: una cadena encadenable de módulos
vocal (EQ, compresor, de-esser, saturación, delay, reverb, limiter, pasa-altos,
ganancia) con presets aplicables en vivo, bypass por módulo y global, y niveles
de salida pre/post. La **Fase 1.1** añade el ajuste fino del ecualizador por
banda (sliders en vivo). La **Fase 2** añade el asistente vocal local: análisis
de la voz en vivo (sin FFT ni nube), sugerencias accionables con confirmación y
resumen de sesión exportable.

## Principios

- **Latencia ante todo**: el pipeline de audio evita bloqueos y asignaciones en
  el camino crítico; la latencia se mide y se reporta en tiempo real.
- **Núcleo desacoplado**: toda la lógica de audio e IA vive en `voxlfa-core`
  (Rust puro). Ni la UI ni Tauri dependen del DSP; el core no depende de ellas.
- **Seguridad por defecto**: la conexión remota exige un código de emparejamiento.
- **Un solo contrato**: el protocolo (Rust/serde) tiene un espejo TypeScript en
  el escritorio y en el móvil. Nunca se cambia un campo sin actualizar los tres.

## Piezas

```
┌────────────┐    eventos (Tauri emit)     ┌───────────────────────┐
│  Cabina    │ ◄─────────────────────────── │  voxlfa-desktop       │
│  (React)   │                              │  ──────────────────   │
└────────────┘   comandos (invoke) ───────► │  EngineManager        │
                                            │  pairing + WS server  │
                                            └──────────┬────────────┘
        ┌──────────────────────────────────────────────┘  eventos (JSON,
        │                                        WebSocket autenticado)
┌───────▼────────┐        canal mpsc       ┌───────────────┐
│ voxlfa-core    │ ──────────────────────► │ hilo forwarder│──► UI + WS
│ (motor de      │   callbacks de audio    └───────────────┘
│  audio + DSP)  │   (sin bloqueos)
└───────┬────────┘
        │  ring buffer (lock-free)
        ▼
   dispositivo de salida
```

### `core/` — voxlfa-core (crate Rust)

El motor de audio y, en fases futuras, el DSP y la IA. Publica:

- `audio::AudioEngine`: crea los streams de entrada y salida con `cpal`,
  conecta captura → salida mediante un **ring buffer lock-free** (`ringbuf`)
  y mide la latencia como el tiempo que tarda cada muestra en recorrerlo. El
  tamaño de buffer es configurable; si no se pide, lo elige una **heurística
  por dispositivo** (USB → 128, Bluetooth/HDMI → 1024, resto → 256).
- `audio::EngineHandle`: control asíncrono del motor (`request_stop`,
  `join`) sin tocar los streams desde fuera del hilo de audio.
- `dsp`: trait `AudioProcessor` + procesadores reales:
  - **Filtros**: `BiquadFilter`/`BiquadKind` (peaking, low-shelf, high-shelf,
    paso banda, notch), `HighPass`, `Notch` (muesca antifeedback),
    `ParametricEq` (hasta 11 bandas).
  - **Dinámica**: `Compressor` (envolvente pico, ganancia suavizada en dB),
    `DeEsser` (banda de 6 kHz con envolvente), `Limiter` (con lookahead),
    `BoomSuppressor` (reducción dinámica de la banda baja-media ~200–300 Hz).
  - **Tiempo/color**: `Saturator` (tanh), `Delay` (feedback, mezcla),
    `Reverb` (Schroeder: 4 comb + 2 allpass), `Gain`, `PassThroughProcessor`.
  - **Cadena**: `ChainProcessor` encadena módulos en orden, mide latencia
    acumulada y aplica bypass por módulo o global; `DspHandle` permite
    reconfigurar en vivo (`DspCommand`: aplicar preset, bypass global, bypass
    de módulo y reemplazo de un módulo completo) conmutando cadenas o
    procesadores preconstruidos en un hilo de control. El **ajuste fino del
    EQ** (`DspHandle::set_eq_band`) reconstruye solo el `ParametricEq` con la
    banda modificada y lo conmuta por puntero; las bandas actuales viajan en
    el estado (`DspLinkState::eq_bands`).
- `dsp::presets::PresetFactory`: `vozLimpia`, `radio` y `warm` (todas terminan
  en un limiter de seguridad e incluyen antifeedback: pasa-altos + muesca y/o
  supresión de *boominess*).
- `protocol`: contratos serde de eventos y comandos (ver `docs/protocolo.md`),
  incluida la especificación DSP (`protocol/dsp.rs`) que es la única fuente de
  configuración JSON, y el análisis vocal (`protocol/analysis.rs`).
- `analysis`: asistente vocal local (Fase 2):
  - `bands::BandSplitter` divide la señal en bandas con **biquads fijos**
    (graves <200 Hz, baja-media ~300 Hz, media ~1.2 kHz, agudos >4 kHz) y
    acumula energías, picos y cruces por cero. Corre en el callback de audio:
    **O(n) y sin asignación**; cada `ANALYSIS_FRAME_INTERVAL` (200 ms) extrae
    un `VoiceFrame`.
  - `analyzer::VoiceAnalyzer` desliza una ventana de marcos (2 s) y deriva
    métricas: RMS, rango dinámico, factor de cresta, brillo, resonancia y
    fatiga vocal (heurística de esfuerzo sostenido). `SessionTracker` acumula
    las mismas métricas para el resumen de la sesión.
  - `suggest::SuggestionEngine` evalúa reglas heurísticas sobre las métricas y
    produce `Suggestion` con severidad 0–1, mensaje en español y una acción
    confirmable (aplicar un preset o informativa).
  - `handle::AnalysisHandle` expone a la UI la última muestra, el resumen de
    sesión y `apply_suggestion` (delega en `DspHandle` para reconfigurar en
    vivo). El estado compartido (`AnalysisShared`) lo escribe el hilo de
    análisis.

Los callbacks de `cpal` **no asignan memoria ni bloquean mutex**: solo
intercambian el puntero de la cadena activa, procesan el bloque en *scratch*
preasignados, acumulan muestras de nivel y de bandas, y drenan por canal a
hilos dedicados.

### `desktop/` — voxlfa-desktop (Tauri v2)

Cáscara de escritorio que orquesta el core:

- `src-tauri/src/engine.rs`: `EngineManager` mantiene el estado compartido y un
  hilo *forwarder* que consume el canal del motor y reenvía cada evento a la UI
  (vía `app.emit`) y al WebSocket (vía broadcast serializado).
- `src-tauri/src/ws.rs`: servidor WebSocket local (puerto `4356`) que difunde
  los eventos del motor a la app móvil. El handshake **exige** el código de
  emparejamiento en la URL; sin él responde `401`.
- `src-tauri/src/pairing.rs`: genera códigos de 6 caracteres sin caracteres
  ambiguos (`0/O`, `1/I/l`).
- `src-tauri/src/tauri_app.rs` (feature `webview`): comandos expuestos a la UI,
  incluidos `apply_preset`, `set_global_bypass`, `set_link_bypass` y
  `set_eq_band`, que reconfiguran la cadena DSP en vivo vía `EngineManager`, y
  los de análisis: `get_analysis`, `get_session_summary` y `apply_suggestion`.

La UI (React/TS) accede a Tauri **solo** a través de `src/lib/tauri.ts`; el
estado se consume con el hook `useEngine` (que también replica la cabina con un
mock en navegador para previsualizar sin Tauri). La lógica de emparejamiento del
escritorio es solo de lectura: el código se genera una vez y se muestra.

### `mobile/` — VoxLFA Monitor (Expo/React Native)

Aplicación de **monitoreo remoto** (no procesa audio). Se conecta al WebSocket
del escritorio en la red local con la IP, el puerto y el código de
emparejamiento. Muestra estado, latencia, niveles pre/post de la cadena DSP y el
preset activo en tiempo real, con reconexión automática por retroceso
exponencial.

## Flujo de eventos

1. `cpal` invoca los callbacks con bloques de audio.
2. El core procesa el bloque con la cadena DSP activa (puntero conmutado por
   `DspHandle`, sin bloqueos), copia las muestras al ring buffer, acumula
   energías de banda en el `BandSplitter` y publica métricas de nivel de
   entrada **y** salida.
3. Un hilo dedicado en el core drena métricas y emite `EngineEvent` por canal
   `mpsc` (niveles cada 50 ms; el estado en cada transición). Cada 200 ms el
   `BandSplitter` extrae un `VoiceFrame` a otro hilo dedicado de análisis.
4. El hilo de análisis desliza la ventana, evalúa sugerencias, mantiene el
   resumen de sesión (consultable vía `AnalysisHandle`) y emite
   `EngineEvent::Analysis` (máx. cada ~500 ms).
5. Cuando el estado DSP cambia (preset, bypass o ajuste de EQ), el motor emite
   `EngineEvent::Dsp` con la nueva cadena (incluidas las bandas del EQ); el
   escritorio lo difunde igual que el resto.
6. `EngineManager` (forwarder) actualiza el estado compartido y reenvía:
   - a la UI como evento Tauri `engine-event`;
   - al WebSocket como JSON serializado.
7. El móvil valida el JSON con su guard de tipos y actualiza la vista.

## Reconfiguración DSP en vivo

La cadena no se toca desde el hilo de audio:

1. La UI llama `apply_preset`/`set_*_bypass`/`set_eq_band` → `DspCommand` por
   canal mpsc.
2. El hilo de control de `DspHandle` construye la cadena (o el módulo EQ)
   nueva —aquí sí se puede asignar memoria— y la intercambia atómicamente con
   la activa. Para el EQ fino solo se reemplaza el procesador del eslabón `eq`
   (`SetLinkProcessor`), sin reconstruir reverb/delay ni perder su estado.
3. El callback de audio solo ve el puntero nuevo en la siguiente iteración;
   actualiza `Arc<Mutex<DspState>>` y emite `EngineEvent::Dsp`.

## Persistencia y perfiles por dispositivo

La configuración de la cabina se guarda en `config.json` y se reaplica al
volver a conectar el mismo dispositivo:

1. `voxlfa-core/config.rs` define el esquema (`AppConfig` + `DeviceProfile`) y
   un `ConfigStore` tolerante a fallos (archivo ausente/corrupto → vacío).
   La ruta es `$XDG_CONFIG_HOME/voxlfa/config.json` (o `~/.config/…`).
2. `EngineManager` (desktop) orquesta la persistencia:
   - Al **arrancar**, aplica el perfil del dispositivo elegido (preset como
     `initial_preset` y, después de levantar el pipeline, `set_eq_bands` + los
     bypasses) — reaplica el ajuste fino con un único intercambio del EQ.
   - Al **cambiar preset/bypasses**, los persiste al instante.
   - El **ajuste fino del EQ** se actualiza en memoria y se vuelca al detener
     el motor (evita escribir el archivo en cada paso del slider).
3. `get_config` expone la configuración a la UI para precargar los selectores
   (validando que el dispositivo siga conectado).

La clave de perfil es el nombre del dispositivo de entrada (o `"default"` si se
usa el predeterminado del sistema): no hay heurística de identificación más
fiable y documenta la limitación ante cambios de nombre/puerto USB.

## Latencia

La latencia captura→salida se mide como el número de muestras en el ring
buffer dividido por la frecuencia de muestreo:

```
latency_ms = ring.occupied_len() / sample_rate * 1000
```

El buffer del puente se dimensiona a `RING_CAPACITY_SECS` (2 s) como respaldo
de seguridad frente a *underruns*, aunque en operación normal el nivel ocupado
es mínimo (la métrica real se lee en cada bloque).

## Decisiones de implementación

| Decisión | Justificación |
| --- | --- |
| Monorepo con workspace Cargo (core + desktop) | Compilar/probar todo con un comando |
| `crate-type = ["cdylib", "rlib"]` | Reutilizable por FFI/móvil en el futuro |
| Ring buffer lock-free entre callbacks | Sin contención en el camino de audio |
| Niveles con umbral y drenado por canal | Los callbacks nunca hacen trabajo lento |
| Cadena conmutada por puntero (hilo de control) | Reconfiguración en vivo sin bloquear audio |
| Reemplazo del módulo EQ solo (`SetLinkProcessor`) | Ajuste fino sin reconstruir la cadena entera ni perder el estado de reverb/delay |
| Bandas del EQ en el estado (`eqBands`) | La UI y el móvil ven la configuración real, no solo el preset |
| Análisis sin FFT (bandas con biquads) | O(n), sin dependencia extra y sin asignación en el callback |
| Análisis en hilo dedicado + canal acotado | Las sugerencias y los `String` no tocan el camino de audio |
| `DspCommand` sin `Debug` | Evita `unwrap`/`expect` y simplifica el patrón |
| Perfiles por dispositivo de entrada (nombre) | Recuerda preset/EQ/bypass al reconectar el mismo dispositivo |
| `set_eq_bands` (reaplica el perfil al arrancar) | Un solo intercambio del EQ, sin reconstruir el resto de la cadena |
| EQ fino en memoria + volcado al detener | No escribe `config.json` en cada paso del slider |
| Config tolerante a fallos (best-effort) | Un archivo corrupto nunca impide arrancar la sesión |
| WebSocket con token en la URL | Autenticación simple y sin estado |
| Tipos espejo en tres lenguajes | Contrato único y verificable |

## Límites y pendientes

- El análisis usa **reglas heurísticas** sobre bandas de biquads (sin FFT ni
  modelo entrenado); los umbrales de `suggest.rs` se afinan con voz real.
- La UI aplica **presets existentes** a partir de las sugerencias; la
  generación de presets a medida queda para una fase posterior.
- El móvil **solo monitorea** el análisis y las sugerencias (el bloque 5,
  control desde el móvil con autenticación mutua, quedó fuera de la Fase 2).
- El perfil se indexa por el **nombre del dispositivo de entrada**: si el
  sistema cambia el nombre (p. ej. al mover un USB de puerto), el perfil se
  pierde para ese dispositivo (no hay identificación por hardware).
- Sin autodetección de la IP del escritorio para el móvil.
