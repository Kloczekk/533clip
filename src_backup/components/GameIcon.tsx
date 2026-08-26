import {
  IconGameBall,
  IconGameBlock,
  IconGameCar,
  IconGameChat,
  IconGameController,
  IconGameCrosshair,
  IconGameGlobe,
  IconGameHome,
  IconGameMusic,
  IconGameRocket,
  IconGameSword,
} from "./Icons";

// Small curated category map for sidebar/library readability — not an
// exhaustive game database (no network lookups, stays local-first) and
// deliberately generic icons rather than recreations of any game's actual
// logo/branding.
const EXACT_CATEGORY: Record<string, string> = {
  roblox: "block",
  "roblox studio": "block",
  minecraft: "block",
  valorant: "crosshair",
  "counter-strike 2": "crosshair",
  cs2: "crosshair",
  csgo: "crosshair",
  "counter-strike": "crosshair",
  "apex legends": "crosshair",
  overwatch: "crosshair",
  "overwatch 2": "crosshair",
  "call of duty": "crosshair",
  warzone: "crosshair",
  "rainbow six siege": "crosshair",
  "league of legends": "sword",
  "dota 2": "sword",
  "world of warcraft": "sword",
  terraria: "sword",
  tmodloader: "sword",
  "grand theft auto v": "car",
  gta5: "car",
  "gta v": "car",
  "rocket league": "car",
  fortnite: "rocket",
  "among us": "rocket",
  destiny2: "rocket",
  "the sims 4": "home",
  fifa: "ball",
  discord: "chat",
  chrome: "globe",
  spotify: "music",
};

const SUBSTRING_CATEGORY: [string, string][] = [
  ["roblox", "block"],
  ["minecraft", "block"],
  ["terraria", "sword"],
  ["counter-strike", "crosshair"],
  ["valorant", "crosshair"],
  ["overwatch", "crosshair"],
  ["call of duty", "crosshair"],
  ["league of legends", "sword"],
  ["grand theft auto", "car"],
  ["rocket league", "car"],
  ["fortnite", "rocket"],
];

const ICONS = {
  block: IconGameBlock,
  crosshair: IconGameCrosshair,
  sword: IconGameSword,
  car: IconGameCar,
  rocket: IconGameRocket,
  home: IconGameHome,
  ball: IconGameBall,
  chat: IconGameChat,
  globe: IconGameGlobe,
  music: IconGameMusic,
  controller: IconGameController,
} as const;

function categoryFor(name: string): keyof typeof ICONS | null {
  const key = name.trim().toLowerCase();
  if (!key || key === "general" || key === "ungrouped") return null;
  if (EXACT_CATEGORY[key]) return EXACT_CATEGORY[key] as keyof typeof ICONS;
  const match = SUBSTRING_CATEGORY.find(([needle]) => key.includes(needle));
  return match ? (match[1] as keyof typeof ICONS) : "controller";
}

export function GameIcon({
  name,
  size = 14,
  className,
}: {
  name: string;
  size?: number;
  className?: string;
}) {
  const category = categoryFor(name);
  if (!category) return null;
  const Icon = ICONS[category];
  return <Icon size={size} className={className} />;
}
