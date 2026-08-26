import type { Clip } from "../types/clip";
import { gameNameForClip } from "./gameName";

export type ClipGroupLabel = string;

export interface ClipGroup {
  label: ClipGroupLabel;
  clips: Clip[];
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function monthLabel(d: Date, now: Date): string {
  const month = d.toLocaleString(undefined, { month: "long" });
  return d.getFullYear() === now.getFullYear() ? month : `${month} ${d.getFullYear()}`;
}

function dayLabel(d: Date, now: Date): string {
  const t = startOfDay(d).getTime();
  const today = startOfDay(now).getTime();
  const yesterday = today - 86_400_000;
  if (t >= today) return "Today";
  if (t >= yesterday) return "Yesterday";
  return d.toLocaleDateString(undefined, { day: "2-digit", month: "short" });
}

function timeLabel(d: Date): string {
  return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function groupClipsByDate(clips: Clip[]): ClipGroup[] {
  const now = new Date();
  const buckets = new Map<ClipGroupLabel, Clip[]>();
  const order: ClipGroupLabel[] = [];

  function add(label: ClipGroupLabel, clip: Clip) {
    if (!buckets.has(label)) {
      buckets.set(label, []);
      order.push(label);
    }
    buckets.get(label)!.push(clip);
  }

  const sorted = [...clips].sort(
    (a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
  );

  let sessionLabel: string | null = null;
  let sessionGame: string | null = null;
  let sessionLastTime = 0;
  const sessionGapMs = 45 * 60_000;

  for (const clip of sorted) {
    const date = new Date(clip.createdAt);
    const game = gameNameForClip(clip);
    const time = date.getTime();
    if (!sessionLabel || sessionGame !== game || Math.abs(sessionLastTime - time) > sessionGapMs) {
      sessionLabel = `${game} - ${dayLabel(date, now)} ${timeLabel(date)}`;
      if (date.getFullYear() !== now.getFullYear()) {
        sessionLabel = `${game} - ${monthLabel(date, now)} ${timeLabel(date)}`;
      }
      sessionGame = game;
    }
    sessionLastTime = time;
    add(sessionLabel, clip);
  }

  return order.map((label) => ({ label, clips: buckets.get(label)! }));
}
