# Seguridad de VoxLFA

El producto maneja audio en tiempo real y conexiones en red local. Las reglas
de esta página son obligatorias y se verifican en cada revisión.

## Amenazas consideradas

| Amenaza | Impacto | Mitigación |
| --- | --- | --- |
| Conexión anónima al WebSocket | Control/sabotaje remoto | Código de emparejamiento obligatorio |
| Fuerza bruta del código en la red local | Acceso no autorizado | Rotación tras `MAX_FAILED_ATTEMPTS` fallos consecutivos |
| Fuga del código de emparejamiento | Acceso no autorizado | No se loguea; solo se muestra en la UI |
| Entrada malformada por la red | Crash/DoS | Validación de tipos y longitud |
| Ejecución de script en la cabina | Robo de datos del sistema | CSP estricto en Tauri |
| Callback de audio lento | Glitches/underruns | Prohibido asignar/bloquear en callbacks |

## WebSocket

- **Autenticación**: el cliente debe presentar el código de emparejamiento en
  la URL (`?token=...`). El handshake lo valida; si no, responde `401` y cierra.
  La validación ocurre antes de aceptar la conexión, por lo que un cliente sin
  token **no** puede consumir eventos.
- **Rotación por fallos**: el código rota automáticamente tras
  `MAX_FAILED_ATTEMPTS` (3) handshakes fallidos consecutivos (`PairingState`).
  El nuevo código se publica por el evento Tauri `pairing-event` y la cabina lo
  muestra al instante, así un ataque de fuerza bruta en la red local obliga al
  atacante a volver a empezar y queda a la vista del operador. Un acierto
  reinicia el contador.
- **El token equivale a mando remoto**: el cliente autenticado puede detener el
  motor y reconfigurar la cadena (preset, bypasses y EQ). No hay autenticación
  mutua por dispositivo; proteger el código y no compartirlo fuera de la red de
  confianza.
- **No loguear códigos**: el código generado vive en `PairingState` y nunca se
  escribe en logs de producción ni en eventos. Las rotaciones se difunden solo
  por el canal interno `pairing-event` hacia la UI.
- **Longitud**: el token se compara por igualdad exacta con el código generado
  (6 caracteres); la comparación no filtra el valor. Rechaza cualquier entrada
  fuera de ese dominio.
- **Alcance**: el servidor escucha en todas las interfaces de la red local
  (0.0.0.0). No se publica a Internet: documentar al usuario que la app debe
  usarse solo en redes de confianza.
- **Tasas de reintento**: el móvil aplica retroceso exponencial acotado (máx.
  8 s, máx. 5 reintentos) para no saturar el escritorio con reintentos. Ojo: un
  reintento constante también rota el código (ver arriba), así que el móvil debe
  pedir el código nuevo al usuario si lo ha perdido.

### Entradas de control (comandos del móvil)

Todo comando entrante se valida antes de ejecutarse:

- Tamaño ≤ 1 KB (rechazado con `warning` si lo supera).
- JSON deserializable como `ControlCommand` (rechazado con `warning` si no).
- `start` se rechaza: arrancar el motor solo desde la cabina.
- La ganancia del EQ se acota a `[-18, 18]` dB; el resto de rangos los valida el
  motor (índices de banda, nombres de módulo, preset).

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

- Cifrado en tránsito si el WebSocket sale de la red local (wss + TLS).
- Autenticación mutua del móvil (clave por dispositivo): se evaluó y se
  descartó por ahora; el código de emparejamiento sigue siendo el único factor
  que autoriza el control remoto (ver sección WebSocket).
