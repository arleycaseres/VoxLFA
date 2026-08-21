# VoxLFA

**Procesador vocal en vivo con IA.** Toma el audio de un micrófono, lo limpia y
lo mejora en tiempo real — elimina feedback/Larsen, reduce ruido de fondo,
mejora la claridad vocal y corrige el tono — con latencia suficientemente baja
para uso en directo (conciertos, iglesias, karaoke, streaming).

100% software, corre local (sin depender de internet) y usa modelos de IA
livianos en vez de solo DSP clásico.

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
Estado actual: **Fase 9 completada — Efectos multi-modo profesionales** (delay y
reverb con calidad de concierto).

- [x] Monorepo (`core` / `desktop` / `mobile`)
- [x] Pipeline de audio: captura → cadena DSP → salida con medición de latencia
- [x] Módulos DSP: EQ, compresor, de-esser, saturación, limiter
- [x] Delay multi-modo (Digital, Analog, Tape, Slapback) + ducking + pre-delay
- [x] Reverb multi-modo (Plate, Hall, Room) + pre-delay
- [x] Presets aplicables en vivo (Voz Limpia, Radio, Warm) con bypass por módulo
- [x] Denoise ONNX (DeepFilterNet3) offloaded a hilo dedicado
- [x] Supresión de feedback adaptativa + boom suppressor
- [x] Corrección tono (YIN + PSOLA, escalas musicales)
- [x] Asistente IA local (Groq/GPT-OSS-20B, sugerencias contextuales)
- [x] Visualizador de espectro FFT (32 bandas logarítmicas)
- [x] Persistencia por dispositivo (perfiles con EQ, gate, delay, reverb)
- [x] Protocolo de comunicación core ↔ UI (incluido WebSocket para móvil)
- [x] Emparejamiento móvil ↔ escritorio (WebSocket autenticado por token + QR)
- [ ] Fase 10 — Efectos multi-modo: Feedback adaptativo mejorado (FIR), Dynamic EQ, Saturación multi-modo

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
la cadena, y el panel de presets te deja cambiar entre Voz Limpia, Radio y Warm
(con bypass por módulo o global). El indicador de latencia (ms) te dice si el
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
