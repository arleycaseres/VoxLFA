# Seguridad de VoxLFA

El producto maneja audio en tiempo real y conexiones en red local. Las reglas
de esta página son obligatorias y se verifican en cada revisión.

## Amenazas consideradas

| Amenaza | Impacto | Mitigación |
| --- | --- | --- |
| Conexión anónima al WebSocket | Control/sabotaje remoto | Código de emparejamiento obligatorio |
| Fuga del código de emparejamiento | Acceso no autorizado | No se loguea; solo se muestra en la UI |
| Entrada malformada por la red | Crash/DoS | Validación de tipos y longitud |
| Ejecución de script en la cabina | Robo de datos del sistema | CSP estricto en Tauri |
| Callback de audio lento | Glitches/underruns | Prohibido asignar/bloquear en callbacks |

## WebSocket

- **Autenticación**: el cliente debe presentar el código de emparejamiento en
  la URL (`?token=...`). El handshake lo valida; si no, responde `401` y cierra.
  La validación ocurre antes de aceptar la conexión, por lo que un cliente sin
  token **no** puede consumir eventos.
- **No loguear códigos**: el código generado vive en `AppState` y nunca se
  escribe en logs de producción ni en eventos.
- **Longitud**: el token se compara por igualdad exacta con el código generado
  (6 caracteres); la comparación no filtra el valor. Rechaza cualquier entrada
  fuera de ese dominio.
- **Alcance**: el servidor escucha en todas las interfaces de la red local
  (0.0.0.0). No se publica a Internet: documentar al usuario que la app debe
  usarse solo en redes de confianza.
- **Tasas de reintento**: el móvil aplica retroceso exponencial acotado (máx.
  8 s, máx. 5 reintentos) para no saturar el escritorio con reintentos.

## CSP de Tauri

Definido en `desktop/src-tauri/tauri.conf.json`. Actualmente:

```
default-src 'self'
style-src 'self' 'unsafe-inline'
font-src 'self'
img-src 'self' data:
connect-src 'self'
```

- Sin `script-src` externa: **no se cargan scripts de terceros**.
- `font-src 'self'`: las fuentes están auto-hospedadas (sin red al cargar).
- `connect-src 'self'`: la UI solo conecta con el propio backend Tauri.
- Al añadir funciones (telemetría, IA), **ampliar la CSP solo con lo mínimo** y
  revisar que no habilite fuentes innecesarias.

## Entradas de red (validación obligatoria)

Toda entrada que cruce la red (query string, campos JSON) debe validarse:

1. **Tipo**: rechazar si el `type` no es uno de los del protocolo.
2. **Longitud**: acotar strings (p. ej., token ≤ 16, mensajes ≤ 1 KB).
3. **Rangos**: niveles dBFS acotados a `[-120, 0]`; puertos a `1..=65535`.

## Rust: prácticas obligatorias

- `#![forbid(unsafe_code)]` en `voxlfa-core` (no hay `unsafe` en producción).
- Sin `unwrap()`/`expect()` en código de producción: `Result` + `?` con el tipo
  `Error` del crate (`thiserror`).
- En callbacks de `cpal`: **no** asignar memoria, **no** bloquear mutex largos,
  **no** hacer I/O. Se acumula y se drena por canal a un hilo dedicado.

## Revisión en CI/verificación

La verificación final del proyecto ejecuta, además de tests:

- `cargo clippy --workspace --all-targets -- -D warnings` (prohíbe `unsafe`,
  no-op, etc.).
- `cargo test --workspace` (incluye los tests de rechazo del WebSocket).
- `npm run build` y `npx tsc --noEmit` (typecheck estricto de UI y móvil).

## Pendientes (fases futuras)

- Rotación del código de emparejamiento tras N intentos fallidos.
- Cifrado en tránsito si el WebSocket sale de la red local (wss + TLS).
- Autenticación mutua del móvil (clave por dispositivo) si se añade control
  remoto del motor.
