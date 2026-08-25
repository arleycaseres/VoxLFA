# VoxLFA

**Procesador vocal en vivo con IA.** Toma el audio de un micrófono, lo limpia y
lo mejora en tiempo real — elimina feedback/Larsen, reduce ruido de fondo,
mejora la claridad vocal y corrige el tono — con latencia suficientemente baja
para uso en directo (conciertos, iglesias, karaoke, streaming).

100% software, corre local (sin depender de internet) y usa modelos de IA
livianos en vez de solo DSP clásico. Incluye cadena DSP completa con 14
módulos (EQ, compresor, de-esser, saturación multi-modo, dynamic EQ,
harmonizer vocal, delay/reverb multi-modo con enrutamiento send/return,
supresión de feedback adaptativa FIR, denoise ONNX, corrección de tono y
limiter), 6 presets optimizados (Dry, VozLimpia, Radio, Warm, Monitor, FOH)
y monitoreo remoto desde el móvil.

---

## Arquitectura

| Carpeta   | Stack                          | Rol                                                      |
| --------- | ------------------------------ | -------------------------------------------------------- |
| `core/`   | Rust (`cdylib` + `rlib`)       | Motor DSP + IA. El "cerebro", reutilizable en todas las plataformas. |
| `desktop/`| Tauri v2 + React + TypeScript  | App de escritorio (Windows/macOS/Linux). Captura/salida de audio con `cpal`, UI "cabina de instrumento". |
| `mobile/` | Expo / React Native            | App de **monitoreo y control remoto** del motor de escritorio (WebSocket por red local). No procesa audio. |
| `docs/`   | Markdown                       | Documentación de arquitectura, seguridad y protocolo.    |

El núcleo compartido está en `core/`; la UI de escritorio solo es una cáscara
que se comunica con él. El móvil no duplica lógica de audio: se conecta al motor
que corre en el escritorio.

## Cambios recientes

### Harmonizer vocal, Presets Monitor/FOH y Send/Return (Fase 11) ✅

- **Harmonizer**: genera harmonías vocales en tiempo real con pitch shifter por
  delay-lines y crossfade triangular. Soporta hasta 8 intervalos × 4 voces.
  Detuning ±5 cents para efecto coro. UI con presets (Octava, Quinta, Tercera),
  mix slider y voices-per-interval (1–4).
- **Presets Monitor y FOH**: Monitor sin delay/reverb (evita latencia en
  escenario). FOH con cadena completa (dynamic eq, saturador, harmonizer,
  slapback, plate reverb).
- **Enrutamiento Send/Return**: delay y reverb procesan la misma señal seca en
  paralelo, evitando que el reverb procese las colas del delay. Detección
  automática del primer efecto de tiempo en la cadena.
- **Fix freeze audio callback**: corregido el congelamiento progresivo causado
  por (1) armónicos del harmonizer que reasignaban Vec en el hot-path, (2)
  buffers USB genéricos demasiado pequeños (256→512) y (3) reasignaciones en
  `scratch.resize()`/`denoise_in_buf.resize()`/`denoise_out_buf.resize()`
  cuando el dispositivo entregaba callbacks más grandes que el nominal.
  Buffers preasignados a 4096 (~85 ms a 48 kHz).

### Saturación multi-modo, Dynamic EQ y Feedback FIR adaptativo (Fase 10) ✅

- **Saturador**: 3 modos (Tube: armónicos pares, Tape: compresión suave +
  LP, TubeTape: cascada). Drive, mix y filtros biquad en señal wet.
- **Dynamic EQ**: compresión por banda de frecuencia. Extracción paralela con
  biquad pasabanda sidechain, detector de envolvente pico, ratio/attack/release.
- **Feedback FIR adaptativo (NLMS)**: modelo de la ruta de feedback
  (altavoz → micrófono) con cancelación por sustración. Los presets en vivo
  usan modo Adaptive; Radio usa Notch clásico.

### Efectos multi-modo profesionales (Fase 9) ✅

Delay y reverb multi-modo con calidad de concierto:

- **Delay**: 4 modos (Digital limpio, Analog cálido con degradación, Tape vintage
  con wow & flutter, Slapback para ensanchamiento vocal). Incluye pre-delay,
  filtros HP/LP en señal wet, y ducking (el delay se atenúa cuando cantas).
- **Reverb**: 3 modos (Placa densa y brillante para vocales, Sala envolvente
  para espacios grandes, Habitación corta y natural). Incluye pre-delay para
  separar la voz de la cola, y filtros HP/LP en la señal de retorno.
- **Presets actualizados**: VozLimpia (Slapback 65ms + Plate), Radio (Tape 120ms
  + Room), Warm (Digital 80ms + Plate).
- **UI completa**: paneles DelayPanel y ReverbPanel con selectores de modo,
  sliders de todos los parámetros, CSS y conexión a useEngine.

### Mejoras de fases anteriores

- **Noise gate hold**: 25ms → 120ms (trabaja el trino "rrrrr" sin cortar).
- **Buffer USB inteligente**: clasificación por tier (Gama alta=128, media/baja=
  512, genérico=256) en vez de 256 fijo para todos.
- **Denoise offloaded**: inferencia ONNX en hilo dedicado con ring buffers
  (fuera del callback de audio).
- **IA advisor**: prompt comprimido (~1200 tokens), modelo GPT-OSS-20B (1000 TPS),
  reintentos en rate-limit, contenido vacío reportado correctamente.

### Panel de sugerencias flotante

- Barra flotante con toggle show/hide (persiste en localStorage).
- SuggestionCard muestra el panel exacto donde aplicar cada sugerencia.

---

## Roadmap

Ver [`docs/roadmap.md`](docs/roadmap.md) para el detalle completo por fases.
Estado actual: **Fase 11 completada — Harmonizer, Presets Monitor/FOH,
Send/Return FX routing**.

- [x] Monorepo (`core` / `desktop` / `mobile`)
- [x] Pipeline de audio: captura → cadena DSP → salida con medición de latencia
- [x] Módulos DSP: EQ, compresor, de-esser, saturación multi-modo, limiter
- [x] Dynamic EQ (compresión por banda de frecuencia)
- [x] Harmonizer vocal (delay-lines con crossfade, detuning, múltiples voces)
- [x] Delay multi-modo (Digital, Analog, Tape, Slapback) + ducking + pre-delay
- [x] Reverb multi-modo (Plate, Hall, Room) + pre-delay
- [x] Enrutamiento Send/Return (delay+reverb en paralelo)
- [x] Presets aplicables en vivo (Dry, VozLimpia, Radio, Warm, Monitor, FOH) con bypass por módulo
- [x] Denoise ONNX (DeepFilterNet3) offloaded a hilo dedicado
- [x] Supresión de feedback adaptativa (FFT+Notch y FIR NLMS) + boom suppressor
- [x] Corrección tono (YIN + PSOLA, escalas musicales)
- [x] Asistente IA local (Groq/GPT-OSS-20B, sugerencias contextuales)
- [x] Visualizador de espectro FFT (32 bandas logarítmicas)
- [x] Persistencia por dispositivo (perfiles con EQ, gate, delay, reverb, saturador, dynamic eq, harmonizer)
- [x] Protocolo de comunicación core ↔ UI (incluido WebSocket para móvil)
- [x] Emparejamiento móvil ↔ escritorio (WebSocket autenticado por token + QR)
- [ ] Fase 12 — Módulo "Sculpt" (un solo control de tono), Aislamiento de voz en tiempo real

## Requisitos

- **Rust** ≥ 1.77 (`rustup`)
- **Node.js** ≥ 20 y **npm**
- Dependencias de sistema para Tauri (Linux: `libgtk-3-dev`, `libasound2-dev`,
  `libwebkit2gtk-4.1-dev`). Ver [Tauri prerequisites](https://tauri.app/start/prerequisites/).

> En entornos sin `webkit2gtk-4.1` (p. ej. Flatpak sin sudo) el backend de
> escritorio se comprueba sin el feature `webview`:
> `cargo check -p voxlfa-desktop --no-default-features`.

## Inicio rápido (desarrollo)

```bash
# 1. Comprobar el workspace Rust (test + lint)
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings

# 2. Frontend del desktop (instala deps de la UI)
cd desktop && npm install && cd ..

# 3. Correr la app de escritorio (ventana + audio)
cd desktop && npm run tauri dev
```

Al iniciar la app verás el **instrumento de cabina**: el dial central muestra el
nivel de entrada en tiempo real, los medidores muestran los niveles pre/post de
la cadena, y el panel de presets te deja cambiar entre Dry, VozLimpia, Radio,
Warm, Monitor y FOH (con bypass por módulo o global). Paneles dedicados para
ECUADOR, Puerta de ruido, Denoise, Feedback, Corrección de tono, Delay, Reverb,
Saturador, Dynamic EQ y Harmonizer. El indicador de latencia (ms) te dice si el
pipeline cumple el objetivo para uso en vivo. El código de emparejamiento se
muestra en la esquina superior derecha.

### App móvil (monitoreo remoto)

```bash
cd mobile && npm install && npm start
```

En el móvil escribe la IP del escritorio (la muestra la cabina), el puerto
`4356` y el código de emparejamiento. Se conecta por WebSocket y muestra
medidores, estado y latencia con reconexión automática.

## Documentación

| Documento | Contenido |
| --------- | --------- |
| [`docs/arquitectura.md`](docs/arquitectura.md) | Estructura del sistema, decisiones técnicas, flujo de datos de audio. |
| [`docs/protocolo.md`](docs/protocolo.md) | Formato de eventos y comandos core ↔ UI ↔ móvil (schemas JSON). |
| [`docs/seguridad.md`](docs/seguridad.md) | Modelo de amenazas, emparejamiento desktop↔móvil, buenas prácticas. |
| [`docs/roadmap.md`](docs/roadmap.md) | Plan por fases del proyecto, estado y pendientes. |
| [`AGENTS.md`](AGENTS.md) | Estándares de código y comandos para contribuir. |

## Licencia

MIT — ver [LICENSE](LICENSE).
