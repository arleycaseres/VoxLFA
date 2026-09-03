# AGENTS.md — Estándares del proyecto

Este archivo documenta las convenciones, comandos y estándares de calidad para
trabajar en VoxLFA. Léelo antes de tocar cualquier código.

## Idioma

- **Documentación y comentarios:** español.
- **Identificadores de código (nombres, tipos, funciones, variables):** inglés.
- **UI para el usuario final:** español.

## Estructura del monorepo

- `core/` — Motor DSP + IA en Rust. **No** debe depender de la UI ni de Tauri.
- `desktop/` — App Tauri v2 (backend Rust + frontend React/TS).
- `mobile/` — App Expo/React Native de monitoreo remoto.
- `docs/` — Documentación técnica.

## Comandos estándar

| Comando | Uso |
| ------- | --- |
| `cargo build --workspace` | Compila todo el Rust (requiere webkit2gtk-4.1). |
| `cargo test --workspace` | Corre los tests. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint obligatorio sin warnings. |
| `cargo fmt --all -- --check` | Verifica formato. |
| `cd desktop && npm run build` | TypeScript + Vite build. |
| `cd desktop && npm run tauri dev` | Ejecuta la app de escritorio. |
| `cd mobile && npx tsc --noEmit` | Typecheck del móvil. |

**Entornos sin webview** (p. ej. Flatpak sin `webkit2gtk-4.1`): el backend de
escritorio se comprueba sin el feature `webview` (deja de compilar solo Tauri;
el resto, incluido `core/audio`, sigue intacto):

```bash
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
```

**Siempre** ejecuta lint y typecheck después de tocar código (ver sección
"Verificación final").

## Convenciones de código

### Rust (`core`, `desktop/src-tauri`)

- Seguir [API guidelines de Rust](https://rust-lang.github.io/api-guidelines/).
- Documentar con `///` los ítems públicos (`#![warn(missing_docs)]` está
  habilitado en `core`).
- Sin `unwrap()`/`expect()` en código de producción: usar `Result` + `?` con el
  tipo `Error` del crate (`voxlfa_core::Error`, `thiserror`).
- Errores descriptivos en español? **No**: los mensajes de error internos se
  escriben en inglés (es el estándar de logs); la UI traduce si es necesario.
- La lógica de audio en los callbacks de cpal **no asigna memoria** ni hace
  operaciones lentas (bloqueos de mutex largos, syscalls de I/O): se acumula y
  se envía por canal a un hilo dedicado.
- **Verificar tipos contra signatures antes de escribir código.** Si la función
  recibe `Sender<X>`, no pongas `Option<String>` en la tupla. Releer la
  signature de la función destino antes de construir los argumentos.
- **Antes de commit, releer el diff completo** buscando errores obvios de tipos,
  variables sin usar, y lógica incorrecta. No asumir que el código compila solo
  porque lo escribí.

### Errores conocidos de cpal/ALSA

- `snd_pcm_hw_params_set_buffer_size` → `EINVAL (22)`: el driver ALSA rechaza
  un buffer size que cpal reporta como válido (desalineación de periodos). Fix:
  retry automático con `BufferSize::Default` en `start_engine` (ya implementado).
- `alsa::poll()` → `POLLERR`: dispositivo USB incompatible con ALSA directo o
  driver en mal estado. **Puede ser asíncrono**: el stream se abre con éxito
  (`build_input_stream` OK) y el fallo llega después por el callback de error
  `move |err| { ... "input stream" ... }`. Un check solo sobre errores de build
  **no** lo detecta.
  - Fix síncrono (build): retry en cascada con buffer grande (1024/2048).
  - Fix asíncrono: el **probe de arranque** en `AudioEngine::start`
    (`core/src/audio/engine.rs`, `STARTUP_PROBE_MS`, 500 ms) reutiliza el canal
    de errores de los streams (`probe_tx`). Si el callback emite un error
    dentro de la ventana, `start` devuelve `Err` en vez de éxito, y la cascada
    de `start_engine` reintenta.
- Dispositivos USB genéricos (codec, Burr-Brown, etc.) son propensos a estos
  errores. El retry en cascada en `start_engine` las maneja automáticamente:
  heurístico → default → 1024 → 2048 → y luego **otros hosts** (pulseaudio,
  pipewire, jack) con los dispositivos por defecto de ese host (los nombres de
  dispositivo cambian según el host, así que al cambiar de backend se usan los
  defaults, `input_device=None`/`output_device=None`). El `probe_tx`/`probe_rx`
  viven solo en `start`; **no** añadir I/O ni locks largos en los callbacks de
  error — solo `swap` + `send` por canal sin espera.

### TypeScript / React (`desktop/src`)

- `strict: true` en `tsconfig`.
- Componentes funcionales con hooks, tipados explícitamente.
- El acceso a Tauri vive en `src/lib/tauri.ts` (tipado); la UI **no** llama
  `invoke`/`listen` directamente.
- Sin `any`; definir tipos en `src/lib/types.ts` reflejando el protocolo.
- CSS: variables de diseño en `src/styles/tokens.css`. No reinventar colores.

### Protocolo (core ↔ UI ↔ móvil)

- El contrato de datos vive en `core/src/protocol/` (Rust, serde) y su espejo
  TypeScript en `desktop/src/lib/types.ts` y `mobile/src/lib/protocol.ts`.
- Eventos con `tag = "type"`, `rename_all = "camelCase"` en serde; el TS usa
  los mismos nombres en camelCase. **Nunca** cambies un nombre de campo sin
  actualizar los tres lados.

## Verificación final (obligatoria antes de terminar una tarea)

En máquinas con `webkit2gtk-4.1`:

1. `cargo fmt --all` (o al menos `--check`).
2. `cargo clippy --workspace --all-targets -- -D warnings`.
3. `cargo test --workspace`.
4. `cd desktop && npm run build`.
5. `cd mobile && npx tsc --noEmit` (si el móvil tiene TS).

Sin `webkit2gtk-4.1`, sustituir los pasos 2 y 3 por la variante
`--no-default-features` de la sección "Comandos estándar".

Los cambios de `desktop/src-tauri/src/tauri_app.rs` (feature `webview`) no se
compilan en entornos sin webkit: revísalos con especial cuidado o en una
máquina con las dependencias del sistema instaladas.

**IMPORTANTE:** cuando no puedes compilar con `--features webview` (entorno
sin webkit), los tipos en `tauri_app.rs` **no se verifican** con clippy. En
ese caso, **releer cada tipo manualmente contra la signature de la función
destino** antes de escribir. Ejemplo: si `AudioEngine::start` recibe
`mpsc::Sender<EngineEvent>`, no escribas `Option<String>` en la tupla.

## Seguridad (resumen)

Ver `docs/seguridad.md` para el detalle. Reglas mínimas:

- El WebSocket del desktop exige **código de emparejamiento** (no aceptar
  conexiones anónimas en la red local).
- No introducir secretos en el repositorio. No loguear códigos de emparejamiento
  en texto plano en logs de producción.
- Validar el tamaño/longitud de cualquier entrada que venga de la red.
- El CSP de Tauri no debe habilitar fuentes innecesarias.
