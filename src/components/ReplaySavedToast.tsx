import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

interface ReplaySavedPayload {
  message: string;
  savedPath?: string;
}

const DISMISS_MS = 4500;

export function ReplaySavedToast() {
  const [toast, setToast] = useState<ReplaySavedPayload | null>(null);

  useEffect(() => {
    const unlisten = listen<ReplaySavedPayload>("obs://replay-saved", (event) => {
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
        <p className="replay-toast-title">Replay saved</p>
        <p className="replay-toast-detail">{toast.message}</p>
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
