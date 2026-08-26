import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Clip } from "../types/clip";

const RELEASES_URL = "https://github.com/Kloczekk/533clip/releases";

interface DebugPanelProps {
  watchPath: string | null;
  clips: Clip[];
  onRefresh: () => void;
  onCleanup: () => void;
  onRepair: () => void;
}

export function DebugPanel({
  watchPath,
  clips,
  onRefresh,
  onCleanup,
  onRepair,
}: DebugPanelProps) {
  const stats = useMemo(() => {
    const processing = clips.filter((c) => c.status === "processing").length;
    const failed = clips.filter((c) => c.status === "failed").length;
    const missingThumb = clips.filter((c) => c.status === "ready" && !c.thumbnailPath).length;
    return { processing, failed, missingThumb };
  }, [clips]);

  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void invoke<string>("get_app_version").then(setVersion).catch(() => undefined);
  }, []);

  return (
    <section className="settings-section debug-panel">
      <div className="debug-header">
        <h2>Maintenance</h2>
        <div className="debug-actions">
          <button type="button" className="btn ghost small" onClick={onRepair}>
            Repair library
          </button>
          <button type="button" className="btn ghost small" onClick={onCleanup}>
            Cleanup
          </button>
          <button type="button" className="btn ghost small" onClick={onRefresh}>
            Refresh
          </button>
        </div>
      </div>

      <div className="debug-grid">
        <DebugItem label="Watch folder" value={watchPath ?? "Not set"} wide />
        <DebugItem label="Total clips" value={String(clips.length)} />
        <DebugItem label="Processing" value={String(stats.processing)} />
        <DebugItem label="Failed" value={String(stats.failed)} />
        <DebugItem label="Missing thumbs" value={String(stats.missingThumb)} />
        <DebugItem label="Version" value={version ?? "…"} />
        <div className="debug-item">
          <span>Updates</span>
          <button
            type="button"
            className="btn-link"
            onClick={() => void invoke("open_external_url", { url: RELEASES_URL }).catch(() => undefined)}
          >
            Check for updates
          </button>
        </div>
      </div>
    </section>
  );
}

function DebugItem({
  label,
  value,
  wide,
}: {
  label: string;
  value: string;
  wide?: boolean;
}) {
  return (
    <div className={`debug-item ${wide ? "wide" : ""}`}>
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}
