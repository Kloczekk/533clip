import { useState } from "react";

const DOMAIN_MAP: Record<string, string> = {
  roblox: "roblox.com",
  valorant: "playvalorant.com",
  cs2: "counter-strike.net",
  csgo: "counter-strike.net",
  "counter-strike 2": "counter-strike.net",
  discord: "discord.com",
  assetto: "assettocorsa.it",
  ac2: "assettocorsa.it",
  minecraft: "minecraft.net",
  isaac: "bindingofisaac.com",
  "apex legends": "ea.com",
  overwatch: "playoverwatch.com",
  "overwatch 2": "playoverwatch.com",
  "league of legends": "leagueoflegends.com",
  spotify: "spotify.com",
  chrome: "google.com",
  fortnite: "epicgames.com",
};

export function GameIcon({
  name,
  size = 14,
  className,
}: {
  name: string;
  size?: number;
  className?: string;
}) {
  const [imgFailed, setImgFailed] = useState(false);
  const key = name.trim().toLowerCase();
  if (!key || key === "general" || key === "ungrouped") return null;

  let domain = DOMAIN_MAP[key];
  if (!domain) {
    const found = Object.keys(DOMAIN_MAP).find((k) => key.includes(k));
    if (found) domain = DOMAIN_MAP[found];
  }

  if (domain && !imgFailed) {
    return (
      <img
        src={`https://www.google.com/s2/favicons?domain=${domain}&sz=64`}
        width={size}
        height={size}
        className={className}
        style={{ borderRadius: "4px", objectFit: "contain", display: "inline-block" }}
        alt={name}
        onError={() => setImgFailed(true)}
      />
    );
  }

  // Fallback: colored square with first letter — also used when the
  // remote favicon is blocked/offline/unreachable, not just when the
  // game isn't in DOMAIN_MAP.
  const letter = name.charAt(0).toUpperCase();
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;

  return (
    <div
      className={className}
      style={{
        width: size,
        height: size,
        borderRadius: "4px",
        backgroundColor: `hsl(${hue}, 50%, 35%)`,
        color: "white",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        fontSize: size * 0.7,
        fontWeight: "bold",
        lineHeight: 1,
        verticalAlign: "middle",
        marginRight: "0.4rem",
      }}
    >
      {letter}
    </div>
  );
}
