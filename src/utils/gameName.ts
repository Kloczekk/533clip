import type { Clip } from "../types/clip";

const GENERIC_FOLDERS = new Set([
  "captures",
  "clips",
  "recordings",
  "replays",
  "videos",
  "obs",
  "desktop",
]);

export function gameNameForClip(clip: Clip): string {
  if (clip.gameName?.trim()) return aliasedGameName(cleanGameName(clip.gameName));

  const parts = clip.filePath.split(/[/\\]/).filter(Boolean);
  const parent = parts.length > 1 ? parts[parts.length - 2] : "";
  if (parent && !GENERIC_FOLDERS.has(parent.toLowerCase())) {
    return aliasedGameName(cleanGameName(parent));
  }

  const stem = clip.fileName.replace(/\.[^.]+$/, "");
  const match = stem.match(/^(.+?)(?:\s-\s|_\d{4}|\s\d{4}|-\d{4})/);
  return aliasedGameName(cleanGameName(match?.[1] ?? "Ungrouped"));
}

function cleanGameName(name: string): string {
  return name
    .replace(/\b(win64|win32|windows|shipping|client|game)\b/gi, " ")
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase()) || "Ungrouped";
}

function aliases(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem("533clip-game-aliases") ?? "{}");
  } catch {
    return {};
  }
}

export function setGameAlias(from: string, to: string) {
  const source = cleanGameName(from);
  const target = cleanGameName(to);
  if (!source || !target || source === "Ungrouped") return;
  const next = aliases();
  next[source.toLowerCase()] = target;
  localStorage.setItem("533clip-game-aliases", JSON.stringify(next));
}

function aliasedGameName(name: string): string {
  return aliases()[name.toLowerCase()] ?? name;
}
