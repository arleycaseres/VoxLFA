# Arquitectura de VoxLFA

VoxLFA es un **procesador vocal en tiempo real** con IA. La Fase 0 entregó la
base (motor de audio de baja latencia, cabina de escritorio y monitor móvil
remoto); la **Fase 1** añade el DSP real: una cadena encadenable de módulos
vocal (EQ, compresor, de-esser, saturación, delay, reverb, limiter, pasa-altos,
ganancia) con presets aplicables en vivo, bypass por módulo y global, y niveles
de salida pre/post.

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
    de módulo) conmutando cadenas preconstruidas en un hilo de control.
- `dsp::presets::PresetFactory`: `vozLimpia`, `radio` y `warm` (todas terminan
  en un limiter de seguridad e incluyen antifeedback: pasa-altos + muesca y/o
  supresión de *boominess*).
- `protocol`: contratos serde de eventos y comandos (ver `docs/protocolo.md`),
  incluida la especificación DSP (`protocol/dsp.rs`) que es la única fuente de
  configuración JSON.

Los callbacks de `cpal` **no asignan memoria ni bloquean mutex**: solo
intercambian el puntero de la cadena activa, procesan el bloque en *scratch*
preasignados y acumulan muestras de nivel que se drenan por canal a un hilo
dedicado.

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
  incluidos `apply_preset`, `set_global_bypass` y `set_link_bypass`, que
  reconfiguran la cadena DSP en vivo vía `EngineManager`.

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
   `DspHandle`, sin bloqueos), copia las muestras al ring buffer y publica
   métricas de nivel de entrada **y** salida.
3. Un hilo dedicado en el core drena métricas y emite `EngineEvent` por canal
   `mpsc` (niveles cada 50 ms; el estado en cada transición).
4. Cuando el estado DSP cambia (preset o bypass), el motor emite `EngineEvent::Dsp`
   con la nueva cadena; el escritorio lo difunde igual que el resto.
5. `EngineManager` (forwarder) actualiza el estado compartido y reenvía:
   - a la UI como evento Tauri `engine-event`;
   - al WebSocket como JSON serializado.
6. El móvil valida el JSON con su guard de tipos y actualiza la vista.

## Reconfiguración DSP en vivo

La cadena no se toca desde el hilo de audio:

1. La UI llama `apply_preset`/`set_*_bypass` → `DspCommand` por canal mpsc.
2. El hilo de control de `DspHandle` construye la cadena nueva (aquí sí se puede
   asignar memoria) y la intercambia atómicamente con la activa.
3. El callback de audio solo ve el puntero nuevo en la siguiente iteración;
   actualiza `Arc<Mutex<DspState>>` y emite `EngineEvent::Dsp`.

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
| `DspCommand` sin `Debug` | Evita `unwrap`/`expect` y simplifica el patrón |
| WebSocket con token en la URL | Autenticación simple y sin estado |
| Tipos espejo en tres lenguajes | Contrato único y verificable |

## Límites de la Fase 1

- La cadena DSP se configura con **presets y bypass** (los parámetros de cada
  módulo son fijos por preset; el slider fino por banda de EQ queda para una
  fase posterior).
- El móvil solo monitorea; no controla la cadena (se planea en una fase posterior).
- Sin persistencia de configuración ni autodetección de la IP del escritorio.
- La Fase 2 añadirá el asistente de IA (análisis vocal y ajuste automático).
