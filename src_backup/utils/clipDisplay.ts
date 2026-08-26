import type { Clip } from "../types/clip";

export function clipDisplayName(clip: Clip): string {
  if (clip.displayName?.trim()) return clip.displayName.trim();
  return clip.fileName.replace(/\.[^.]+$/, "");
}
