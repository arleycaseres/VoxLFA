// Guía de configuración interactiva — multi-paso para nuevos usuarios.
//
// Muestra cómo conectar hardware físico paso a paso con diagramas
// sencillos y lenguaje accesible. Se abre automáticamente en el primer
// arranque y queda accesible desde el botón "?" de la barra superior.

import { useState } from "react";
import brandMark from "../assets/brand/brand_mark.png";
import "../styles/SetupGuide.css";

interface SetupGuideProps {
  onClose: () => void;
}

const TOTAL_STEPS = 5;

const TIPS: Array<{ icon: string; text: string }> = [
  {
    icon: "🎤",
    text: "<strong>Pro tip:</strong> Si usas un micrófono USB, puedes conectarlo directamente sin necesidad de interfaz de audio.",
  },
  {
    icon: "🎧",
    text: "<strong>Monitoreo:</strong> Usa auriculares para escuchar tu voz procesada sin generar feedback con el micrófono.",
  },
  {
    icon: "🔧",
    text: "<strong>Buffer bajo:</strong> Si sientes retraso, reduce el tamaño del buffer a 128 o 64 muestras para menor latencia.",
  },
  {
    icon: "📱",
    text: "<strong>App móvil:</strong> Puedes monitorear y controlar VoxLFA desde tu teléfono por WiFi (escanea el código de emparejamiento).",
  },
];

export function SetupGuide({ onClose }: SetupGuideProps) {
  const [step, setStep] = useState(0);

  const next = () => {
    if (step < TOTAL_STEPS - 1) setStep(step + 1);
    else onClose();
  };

  const prev = () => {
    if (step > 0) setStep(step - 1);
  };

  return (
    <div className="setup-overlay" onClick={onClose}>
      <div className="setup-card" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="setup-header">
          <div className="setup-header__brand">
            <img src={brandMark} alt="VoxLFA" className="setup-header__logo" />
            <h1 className="setup-header__title">
              Guía de configuración
            </h1>
          </div>
          <button
            className="setup-header__close"
            onClick={onClose}
            aria-label="Cerrar guía"
          >
            ✕
          </button>
        </div>

        {/* Progress */}
        <div className="setup-progress">
          {Array.from({ length: TOTAL_STEPS }, (_, i) => (
            <div
              key={i}
              className={`setup-progress__dot ${
                i === step
                  ? "setup-progress__dot--active"
                  : i < step
                    ? "setup-progress__dot--done"
                    : ""
              }`}
            />
          ))}
          <span className="setup-progress__label">
            {step + 1} / {TOTAL_STEPS}
          </span>
        </div>

        {/* Body */}
        <div className="setup-body">
          {step === 0 && <StepWelcome />}
          {step === 1 && <StepHardware />}
          {step === 2 && <StepConnect />}
          {step === 3 && <StepConfigure />}
          {step === 4 && <StepTips />}
        </div>

        {/* Footer */}
        <div className="setup-footer">
          <span className="setup-footer__hint">
            {step === 0
              ? "Puedes volver a abrir esta guía desde el botón (?) de arriba."
              : "Puedes saltar esta guía en cualquier momento."}
          </span>
          <div className="setup-footer__actions">
            {step > 0 && (
              <button className="btn btn--ghost" onClick={prev}>
                Atrás
              </button>
            )}
            <button
              className={`btn ${step === TOTAL_STEPS - 1 ? "btn--start" : ""}`}
              onClick={next}
            >
              {step === TOTAL_STEPS - 1 ? "¡Entendido!" : "Siguiente"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ─── Paso 0: Bienvenida ─── */
function StepWelcome() {
  return (
    <div className="setup-step">
      <div className="setup-step__number">Bienvenido</div>
      <h2 className="setup-step__title">
        Tu procesador vocal en vivo está listo
      </h2>
      <p className="setup-step__text">
        <strong>VoxLFA</strong> procesa tu voz en tiempo real: reduce ruido,
        corrige el tono, ecualiza y suprime feedback — todo de forma local,
        sin internet.
      </p>
      <p className="setup-step__text">
        Esta guía te enseñará <strong>qué hardware necesitas</strong>, cómo
        <strong>conectarlo</strong> a tu computadora y cómo
        <strong>configurar VoxLFA</strong> para obtener los mejores resultados.
      </p>
      <p className="setup-step__text" style={{ color: "var(--color-cyan)", fontWeight: 600 }}>
        Tardarás menos de 2 minutos.
      </p>
    </div>
  );
}

/* ─── Paso 1: Hardware necesario ─── */
function StepHardware() {
  return (
    <div className="setup-step">
      <div className="setup-step__number">Paso 1</div>
      <h2 className="setup-step__title">¿Qué necesitas?</h2>
      <p className="setup-step__text">
        Necesitas <strong>al menos un micrófono</strong> y <strong>unos auriculares o altavoces</strong>. Eso es todo.
      </p>

      <div className="hw-grid">
        <div className="hw-card">
          <span className="hw-card__icon">🎤</span>
          <span className="hw-card__name">Micrófono</span>
          <span className="hw-card__desc">
            USB, XLR con interfaz, o el micrófono integrado del portátil
          </span>
          <span className="hw-card__badge hw-card__badge--required">Necesario</span>
        </div>

        <div className="hw-card">
          <span className="hw-card__icon">🎧</span>
          <span className="hw-card__name">Auriculares</span>
          <span className="hw-card__desc">
            Para escuchar tu voz procesada sin crear feedback
          </span>
          <span className="hw-card__badge hw-card__badge--required">Recomendado</span>
        </div>

        <div className="hw-card">
          <span className="hw-card__icon">🔊</span>
          <span className="hw-card__name">Altavoces</span>
          <span className="hw-card__desc">
            Funcionan pero pueden causar feedback si están cerca del micrófono
          </span>
          <span className="hw-card__badge hw-card__badge--optional">Opcional</span>
        </div>

        <div className="hw-card">
          <span className="hw-card__icon">🎛️</span>
          <span className="hw-card__name">Interfaz de audio</span>
          <span className="hw-card__desc">
            Mejor calidad y menor latencia. Cualquier interfaz USB sirve
          </span>
          <span className="hw-card__badge hw-card__badge--optional">Recomendado</span>
        </div>
      </div>
    </div>
  );
}

/* ─── Paso 2: Conexión física ─── */
function StepConnect() {
  return (
    <div className="setup-step">
      <div className="setup-step__number">Paso 2</div>
      <h2 className="setup-step__title">Conecta tu hardware</h2>
      <p className="setup-step__text">
        Conecta los cables antes de abrir VoxLFA. Hay dos configuraciones comunes:
      </p>

      <p className="setup-step__text" style={{ fontWeight: 600, color: "var(--color-text)" }}>
        Opción A — Micrófono USB (la más fácil)
      </p>
      <div className="diagram">
        <span className="diagram__node">🎤 Micrófono USB</span>
        {"  "}
        <span className="diagram__arrow">→ USB →</span>
        {"  "}
        <span className="diagram__node">💻 Computadora</span>
        {"  "}
        <span className="diagram__arrow">→</span>
        {"  "}
        <span className="diagram__node">🎧 Auriculares</span>
        {"\n"}
        <span className="diagram__label">
          {"          "}Conecta el micrófono USB a cualquier puerto USB
        </span>
        {"\n"}
        <span className="diagram__label">
          {"          "}Conecta los auriculares a la salida de audio del portátil
        </span>
      </div>

      <p className="setup-step__text" style={{ fontWeight: 600, color: "var(--color-text)" }}>
        Opción B — Micrófono XLR + Interfaz de audio
      </p>
      <div className="diagram">
        <span className="diagram__node">🎤 Micrófono XLR</span>
        {"  "}
        <span className="diagram__arrow">→ XLR →</span>
        {"  "}
        <span className="diagram__node">🎛️ Interfaz</span>
        {"  "}
        <span className="diagram__arrow">→ USB →</span>
        {"  "}
        <span className="diagram__node">💻 Computadora</span>
        {"\n"}
        <span className="diagram__label">
          {"              "}Conecta auriculares a la interfaz (monitor out o headphone out)
        </span>
      </div>

      <ul className="tip-list">
        <li className="tip-list__item">
          <span className="tip-list__icon">⚠️</span>
          <span className="tip-list__text">
            <strong>Importante:</strong> Conecta todo <em>antes</em> de abrir VoxLFA. 
            Si conectas después, haz clic en el botón <strong>Detectar</strong> para refrescar la lista.
          </span>
        </li>
        <li className="tip-list__item">
          <span className="tip-list__icon">💡</span>
          <span className="tip-list__text">
            <strong>Sin micrófono externo:</strong> VoxLFA puede usar el micrófono integrado de tu portátil. 
            La calidad será básica pero suficiente para probar.
          </span>
        </li>
      </ul>
    </div>
  );
}

/* ─── Paso 3: Configurar VoxLFA ─── */
function StepConfigure() {
  return (
    <div className="setup-step">
      <div className="setup-step__number">Paso 3</div>
      <h2 className="setup-step__title">Configura VoxLFA</h2>
      <p className="setup-step__text">
        Abre VoxLFA y sigue estos pasos en el panel izquierdo:
      </p>

      <div className="ui-steps">
        <div className="ui-step">
          <div className="ui-step__num">1</div>
          <div className="ui-step__body">
            <div className="ui-step__label">
              Selecciona la <span className="ui-step__highlight">Entrada</span>
            </div>
            <div className="ui-step__desc">
              En el panel izquierdo "Motor", elige tu micrófono en el menú desplegable "Entrada". 
              Si solo tienes uno, déjalo en "Predeterminado del sistema".
            </div>
          </div>
        </div>

        <div className="ui-step">
          <div className="ui-step__num">2</div>
          <div className="ui-step__body">
            <div className="ui-step__label">
              Selecciona la <span className="ui-step__highlight">Salida</span>
            </div>
            <div className="ui-step__desc">
              Elige tus auriculares o altavoces en "Salida". Si usas auriculares conectados al portátil, 
              el valor por defecto suele ser correcto.
            </div>
          </div>
        </div>

        <div className="ui-step">
          <div className="ui-step__num">3</div>
          <div className="ui-step__body">
            <div className="ui-step__label">
              Haz clic en <span className="ui-step__highlight">Arrancar</span>
            </div>
            <div className="ui-step__desc">
              El botón verde "Arrancar" inicia el procesamiento. Verás los medidores de nivel 
              animarse y la latencia mostrada abajo.
            </div>
          </div>
        </div>

        <div className="ui-step">
          <div className="ui-step__num">4</div>
          <div className="ui-step__body">
            <div className="ui-step__label">
              Habla y ajusta el <span className="ui-step__highlight">preset</span>
            </div>
            <div className="ui-step__desc">
              En el panel derecho, selecciona un preset: <strong>VozLimpia</strong> (buen punto de partida), 
              <strong>Radio</strong> (efecto de locución), o <strong>Warm</strong> (calidez). 
              Ajusta los controles DSP a tu gusto.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ─── Paso 4: Consejos para mejores resultados ─── */
function StepTips() {
  return (
    <div className="setup-step">
      <div className="setup-step__number">Consejos finales</div>
      <h2 className="setup-step__title">Obtén el máximo potencial</h2>

      <ul className="tip-list">
        {TIPS.map((tip, i) => (
          <li key={i} className="tip-list__item">
            <span className="tip-list__icon">{tip.icon}</span>
            <span
              className="tip-list__text"
              dangerouslySetInnerHTML={{ __html: tip.text }}
            />
          </li>
        ))}
      </ul>

      <p className="setup-step__text" style={{ marginTop: "1.2rem" }}>
        <strong>¿Problemas?</strong> Si no escuchas audio o los medidores no se mueven:
      </p>
      <ul className="tip-list">
        <li className="tip-list__item">
          <span className="tip-list__icon">🔌</span>
          <span className="tip-list__text">
            Verifica que el micrófono esté conectado y que lo seleccionaste como entrada.
          </span>
        </li>
        <li className="tip-list__item">
          <span className="tip-list__icon">🔊</span>
          <span className="tip-list__text">
            Revisa que la salida apunte a tus auriculares o altavoces (no a otro dispositivo).
          </span>
        </li>
        <li className="tip-list__item">
          <span className="tip-list__icon">⚙️</span>
          <span className="tip-list__text">
            Haz clic en <strong>Detectar</strong> si conectaste hardware después de abrir la app.
          </span>
        </li>
        <li className="tip-list__item">
          <span className="tip-list__icon">📶</span>
          <span className="tip-list__text">
            Si la latencia es alta, reduce el <strong>buffer</strong> a 128 o 64 muestras.
          </span>
        </li>
      </ul>
    </div>
  );
}
