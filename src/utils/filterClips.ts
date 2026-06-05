import { clipDisplayName } from "./clipDisplay";
import type { Clip } from "../types/clip";

export type LibraryFilter =
  | { kind: "all" }
  | { kind: "favorites" }
  | { kind: "tag"; tag: string };

export function filterClips(
  clips: Clip[],
  filter: LibraryFilter,
  searchQuery: string,
): Clip[] {
  let result = clips;

  switch (filter.kind) {
    case "favorites":
      result = result.filter((c) => c.isFavorite);
      break;
    case "tag":
      result = result.filter((c) => c.tags.includes(filter.tag));
      break;
    default:
      break;
  }

  const q = searchQuery.trim().toLowerCase();
  if (!q) return result;

  return result.filter((c) => {
    const name = clipDisplayName(c).toLowerCase();
    const file = c.fileName.toLowerCase();
    const tags = c.tags.join(" ").toLowerCase();
    return name.includes(q) || file.includes(q) || tags.includes(q);
  });
}
