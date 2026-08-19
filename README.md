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

## Cambios recientes (Frontend)

- Reestructurado el panel de `Sugerencias` para mayor jerarquía y usabilidad:
    - Se separó la sección de `Estado actual` (métricas) y queda colapsada por defecto.
    - `Sugerencias activas` muestra hasta 3 sugerencias ordenadas por severidad, con opción `Ver todas`.
    - Las sugerencias usan una estructura tipada obligatoria: `detected`, `consequence`, `recommendation`, `severity`.
    - El descarte de sugerencias (`dismiss`) persiste en `sessionStorage` durante la sesión.

- Mejora de `Presets`:
    - Lista de presets es ahora un grid responsivo (`auto-fit`/`minmax`) que evita overflow.
    - `PresetCard` reestructurada en columnas (left/right), con truncado seguro y metadatos accesibles.

- Cabecera y logos:
    - Se integraron los logos en `desktop/src/assets/brand/` y se usan en la cabecera.

- IA y sesión:
    - Eliminado botón duplicado `Pedir consejo a la IA` (ahora solo en el panel de IA).
    - `Actualizar resumen` muestra estado de refresco y renderiza campos con comprobaciones seguras.

- Responsividad y accesibilidad:
    - Variables globales para transiciones y breakpoints (`desktop/src/lib/uiConstants.ts` y CSS variables).
    - Botones y controles con áreas táctiles mínimas; SVGs y medidores preparados para escalado.

Pruebas rápidas:

```bash
cd desktop
npm run dev -- --port 1421
```

Abrir http://localhost:1421 y verificar:
- Panel de Sugerencias (máx. 3 visibles, `Ver todas`).
- Presets: nombres largos no generan overflow.
- Cabecera muestra los logos correctamente.

Si detectas problemas visuales indica el componente y la resolución y lo ajustaré.

## Roadmap

Ver [`docs/roadmap.md`](docs/roadmap.md) para el detalle completo por fases.
Estado actual: **Fase 1 completada — DSP en tiempo real** (cadena de módulos
vocal con presets aplicables en vivo y niveles pre/post).

- [x] Monorepo (`core` / `desktop` / `mobile`)
- [x] Pipeline de audio: captura → cadena DSP → salida con medición de latencia
- [x] Módulos DSP: EQ, compresor, de-esser, saturación, delay, reverb, limiter
- [x] Presets aplicables en vivo (Voz Limpia, Radio, Warm) con bypass por módulo
- [x] Protocolo de comunicación core ↔ UI (incluido canal WebSocket para móvil)
- [x] Cabina de escritorio (presets, cadena, dial y medidores pre/post)
- [x] Emparejamiento móvil ↔ escritorio (WebSocket autenticado por token)
- [x] App móvil Expo de monitoreo remoto (estado, latencia, niveles, preset)
- [ ] Fase 2 — IA (ruido, feedback, claridad, tono) — ver roadmap

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
