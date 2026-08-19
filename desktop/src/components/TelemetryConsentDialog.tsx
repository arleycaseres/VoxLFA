export function TelemetryConsentDialog({ onAccept, onDecline }: { onAccept: () => void; onDecline: () => void; }) {
  return (
    <div className="consent-overlay">
      <div className="consent-dialog">
        <h3>Ayuda a mejorar VoxLFA</h3>
        <p>¿Aceptas enviar estadísticas anónimas de uso para mejorar el producto?</p>
        <div style={{ display: "flex", gap: "0.6rem", marginTop: "1rem" }}>
          <button className="btn btn--export" onClick={onAccept}>Aceptar</button>
          <button className="btn btn--ghost" onClick={onDecline}>Rechazar</button>
        </div>
      </div>
    </div>
  );
}

export default TelemetryConsentDialog;
