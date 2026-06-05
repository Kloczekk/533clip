import type { Clip } from "../types/clip";

export type ClipGroupLabel = "Today" | "Yesterday" | "This month" | "Older";

export interface ClipGroup {
  label: ClipGroupLabel;
  clips: Clip[];
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

export function groupClipsByDate(clips: Clip[]): ClipGroup[] {
  const now = new Date();
  const today = startOfDay(now).getTime();
  const yesterday = today - 86_400_000;
  const monthStart = new Date(now.getFullYear(), now.getMonth(), 1).getTime();

  const buckets: Record<ClipGroupLabel, Clip[]> = {
    Today: [],
    Yesterday: [],
    "This month": [],
    Older: [],
  };

  for (const clip of clips) {
    const t = startOfDay(new Date(clip.createdAt)).getTime();
    if (t >= today) buckets.Today.push(clip);
    else if (t >= yesterday) buckets.Yesterday.push(clip);
    else if (t >= monthStart) buckets["This month"].push(clip);
    else buckets.Older.push(clip);
  }

  return (["Today", "Yesterday", "This month", "Older"] as ClipGroupLabel[])
    .map((label) => ({ label, clips: buckets[label] }))
    .filter((g) => g.clips.length > 0);
}
