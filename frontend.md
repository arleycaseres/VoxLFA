Dirección de interfaz

Lo que hace icónico el tablero del LFA no es solo que sea digital, es que la aguja se mueve dentro del display — es un instrumento de precisión que se siente vivo. Ese es el elemento que puede ser tu firma visual: en vez de un medidor de barras genérico, un dial circular central que representa el nivel/dinámica de la voz en tiempo real, con el mismo lenguaje de "instrumento de cabina" que el LFA.

Voy a plantear un sistema de diseño concreto y te muestro un mockup:

Color: negro carbón (
#0B0D0F) como base, paneles en grafito (
#16181C), acento naranja LFA/Nürburgring (
#FF4A1F) para elementos críticos (feedback, alertas), un cian frío (
#4FD8FF) para el espectro/ondas — como el brillo de un display digital de cabina
Tipografía: una fuente técnica/geométrica para números y títulos (estilo instrumento de carrera), sans limpia para etiquetas de UI, monoespaciada para lecturas de dB/Hz
Layout: el dial circular como pieza central (tu "instrumento"), con paneles laterales tipo "cabina" para presets y sugerencias de IA.
diseño en 3d.

0. Identidad de marca y estilo visual
Nombre: VoxLFA — la sigla LFA se resignifica para este producto (ej. "Live Feedback Annihilator"), como guiño a la Lexus LFA sin usar la marca directamente
Estilo de interfaz: "Cockpit", inspirado en el tacómetro digital de la LFA (aguja física sobre pantalla TFT — la mezcla de lo mecánico y lo digital):
Medidores tipo tacómetro/dial en vez de barras verticales genéricas, para nivel de entrada/salida y espectro
Fondo oscuro con textura sutil tipo fibra de carbono
Un solo color de acento fuerte (rojo o naranja racing) reservado para alertas y picos — nunca de fondo
Tipografía técnica/monoespaciada para números (Hz, dB, ms)
Paneles de bordes angulares/cortados, no cajas redondeadas suaves
El espectro de frecuencias (Fase 5) se integra visualmente como si fuera el "RPM" de la voz
\
## Actualizaciones UI (Sugerencias y Responsividad)

Se ha implementado una refactorización del panel de `Sugerencias` y del layout frontend para mejorar la especificidad del feedback de IA, la jerarquía de la UI y la responsividad.

- Sugerencias estructuradas: la UI ahora consume sugerencias en un formato obligatorio que incluye: `detected` (dato objetivo con valor opcional y unidad), `consequence` (por qué importa), `recommendation` (qué hacer) y `severity` (critical/recommended/optional). Esto evita mensajes libres, mejora consistencia y permite badges textuales.
- Panel reorganizado: las métricas quedan colapsadas/compactas por defecto; las `Sugerencias activas` son el foco principal y muestran máximo 3 ítems con opción `Ver todas`. Las sugerencias se pueden descartar y el descarte persiste durante la sesión (`sessionStorage`).
- Footer separado: `Resumen de sesión` y `Pedir consejo a la IA` se colocan en un footer con menor peso visual y border-top.
- Responsividad abusiva: el layout principal usa `grid-template-columns: repeat(auto-fit, minmax(280px,1fr))` para fluir de 3→2→1 columnas sin breakpoints hardcodeados; diales SVG y espectro escalan con `viewBox` y `width:100%`; controles táctiles tienen mínimo de 44×44 px.

Archivos modificados/añadidos relevantes:
- `desktop/src/components/SuggestionPanel.tsx` (reestructuración y mapeo de sugerencias)
- `desktop/src/components/SuggestionCard.tsx` (nuevo componente)
- `desktop/src/components/SuggestionList.tsx` (nuevo componente)
- `desktop/src/components/SessionSummaryFooter.tsx` (nuevo componente)
- `desktop/src/lib/uiTypes.ts` (tipos UI para sugerencias)
- `desktop/src/lib/uiConstants.ts` (transiciones y breakpoints)
- `desktop/src/App.css` (reglas de grid responsivo y estilos nuevos)

Nota: El protocolo entre `core` y `desktop` no se ha modificado; las transformaciones de mensajes se realizan en el frontend para garantizar compatibilidad hacia atrás.