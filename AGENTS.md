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

## Seguridad (resumen)

Ver `docs/seguridad.md` para el detalle. Reglas mínimas:

- El WebSocket del desktop exige **código de emparejamiento** (no aceptar
  conexiones anónimas en la red local).
- No introducir secretos en el repositorio. No loguear códigos de emparejamiento
  en texto plano en logs de producción.
- Validar el tamaño/longitud de cualquier entrada que venga de la red.
- El CSP de Tauri no debe habilitar fuentes innecesarias.
