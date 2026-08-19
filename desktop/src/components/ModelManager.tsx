// Panel de gestión de modelos ONNX: estado de descarga y botón de descarga.
//
// DeepFilterNet3 requiere tres archivos ONNX + config.ini que se descargan
// bajo demanda desde los assets de una release de GitHub.

import type { ModelStatus } from "../lib/types";

interface ModelManagerProps {
  modelStatus: ModelStatus | null;
  downloadProgress: { step: number; total: number } | null;
  onCheckStatus: () => void;
  onDownload: () => void;
}

export function ModelManager({
  modelStatus,
  downloadProgress,
  onCheckStatus,
  onDownload,
}: ModelManagerProps) {
  const downloading = downloadProgress !== null;

  return (
    <div className="modelmanager">
      <div className="modelmanager__header">
        <span className="modelmanager__title">Modelos ONNX (DeepFilterNet3)</span>
        {modelStatus && (
          <span
            className={`modelmanager__badge ${
              modelStatus.available
                ? "modelmanager__badge--ok"
                : "modelmanager__badge--missing"
            }`}
          >
            {modelStatus.available ? "Listo" : "Faltan archivos"}
          </span>
        )}
      </div>

      {modelStatus && !modelStatus.available && (
        <p className="modelmanager__detail">
          Faltan: {modelStatus.missing.join(", ")}
        </p>
      )}

      {modelStatus?.available && (
        <p className="modelmanager__detail">
          Modelos instalados en: {modelStatus.modelDir}
        </p>
      )}

      {downloading && (
        <div className="modelmanager__progress">
          <div className="modelmanager__progress-bar">
            <div
              className="modelmanager__progress-fill"
              style={{
                width: `${downloadProgress!.total > 0 ? (downloadProgress!.step / downloadProgress!.total) * 100 : 0}%`,
              }}
            />
          </div>
          <span className="modelmanager__progress-label">
            Descargando {downloadProgress!.step + 1} de {downloadProgress!.total}…
          </span>
        </div>
      )}

      <div className="modelmanager__actions">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          disabled={downloading}
          onClick={onCheckStatus}
        >
          Actualizar
        </button>
        <button
          type="button"
          className={`btn ${modelStatus?.available ? "btn--ghost" : "btn--start"} btn--sm`}
          disabled={downloading || (modelStatus?.available ?? false)}
          onClick={onDownload}
        >
          {downloading
            ? "Descargando…"
            : modelStatus?.available
              ? "Descargado"
              : "Descargar modelos"}
        </button>
      </div>
    </div>
  );
}
