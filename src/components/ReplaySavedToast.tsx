import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

interface ReplaySavedPayload {
  fileName: string;
  filePath: string;
  gameName?: string;
}

const DISMISS_MS = 4500;

export function ReplaySavedToast() {
  const [toast, setToast] = useState<ReplaySavedPayload | null>(null);

  useEffect(() => {
    const unlisten = listen<ReplaySavedPayload>("clip://detected", (event) => {
      setToast(event.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!toast) return;
    const id = window.setTimeout(() => setToast(null), DISMISS_MS);
    return () => window.clearTimeout(id);
  }, [toast]);

  if (!toast) return null;

  return (
    <div className="replay-toast" role="status" aria-live="polite">
      <span className="replay-toast-icon" aria-hidden>
        ●
      </span>
      <div className="replay-toast-body">
        <p className="replay-toast-title">
          Clipping {toast.gameName?.trim() || "game"}
        </p>
        <p className="replay-toast-detail">{toast.fileName}</p>
      </div>
      <button
        type="button"
        className="replay-toast-close"
        aria-label="Dismiss"
        onClick={() => setToast(null)}
      >
        ×
      </button>
    </div>
  );
}
