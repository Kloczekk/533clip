import type { ReactNode, SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function Icon({ size = 20, children, ...props }: IconProps & { children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      {...props}
    >
      {children}
    </svg>
  );
}

export function IconBack(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M15 18l-6-6 6-6" />
    </Icon>
  );
}

export function IconPlay(props: IconProps) {
  return (
    <Icon {...props}>
      <polygon points="8,5 19,12 8,19" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconPause(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="7" y="5" width="4" height="14" fill="currentColor" stroke="none" />
      <rect x="13" y="5" width="4" height="14" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconSkipBack(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M11 19V5l-7 7 7 7z" />
      <path d="M4 5v14" />
    </Icon>
  );
}

export function IconSkipForward(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13 5v14l7-7-7-7z" />
      <path d="M20 5v14" />
    </Icon>
  );
}

export function IconRewind(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M11 19V5l-7 7 7 7z" />
      <path d="M18 19V5l-7 7 7 7z" />
    </Icon>
  );
}

export function IconForward(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13 5v14l7-7-7-7z" />
      <path d="M6 5v14l7-7-7-7z" />
    </Icon>
  );
}

export function IconScissors(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="6" cy="6" r="3" />
      <circle cx="6" cy="18" r="3" />
      <path d="M20 4L8.5 15.5" />
      <path d="M14.5 8.5L20 14" />
      <path d="M8.5 8.5L12 12" />
    </Icon>
  );
}

export function IconStar(props: IconProps & { filled?: boolean }) {
  const { filled, ...rest } = props;
  return (
    <Icon {...rest}>
      <polygon
        points="12,2 15,9 22,9 17,14 19,22 12,18 5,22 7,14 2,9 9,9"
        fill={filled ? "currentColor" : "none"}
      />
    </Icon>
  );
}

export function IconTrash(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3 6h18" />
      <path d="M8 6V4h8v2" />
      <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6" />
      <path d="M10 11v6M14 11v6" />
    </Icon>
  );
}

export function IconTag(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M20 13l-7 7-10-10V3h7l10 10z" />
      <circle cx="7.5" cy="7.5" r="1.2" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconPencil(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 013 3L7 19l-4 1 1-4 12.5-12.5z" />
    </Icon>
  );
}

export function IconFullscreen(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 3H3v5" />
      <path d="M16 3h5v5" />
      <path d="M21 16v5h-5" />
      <path d="M3 16v5h5" />
    </Icon>
  );
}

export function IconZap(props: IconProps) {
  return (
    <Icon {...props}>
      <polygon points="13,2 4,14 11,14 9,22 20,9 12,9" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconPalette(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3a9 9 0 100 18c1.1 0 2-.9 2-2 0-.5-.2-1-.5-1.35-.3-.35-.5-.8-.5-1.3 0-1.1.9-2 2-2h2a4 4 0 004-4c0-4.4-4.03-7.35-9-7.35z" />
      <circle cx="7.5" cy="12" r="1.2" fill="currentColor" stroke="none" />
      <circle cx="9.5" cy="8" r="1.2" fill="currentColor" stroke="none" />
      <circle cx="14.5" cy="8" r="1.2" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconCloud(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M7 18a4 4 0 01-.5-7.97A5 5 0 0116.9 9.02 4.5 4.5 0 0117 18H7z" />
    </Icon>
  );
}

export function IconDatabase(props: IconProps) {
  return (
    <Icon {...props}>
      <ellipse cx="12" cy="5" rx="7" ry="2.5" />
      <path d="M5 5v14c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5V5" />
      <path d="M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5" />
    </Icon>
  );
}

export function IconCamera(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 8h3l1.5-2h7L17 8h3a1 1 0 011 1v9a1 1 0 01-1 1H4a1 1 0 01-1-1V9a1 1 0 011-1z" />
      <circle cx="12" cy="13" r="3.5" />
    </Icon>
  );
}

export function IconSpeaker(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5 9v6h4l5 4V5L9 9H5z" />
      <path d="M17 9a4 4 0 010 6" />
    </Icon>
  );
}

export function IconKeyboard(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2.5" y="6" width="19" height="12" rx="2" />
      <path d="M6 10h.01M9 10h.01M12 10h.01M15 10h.01M18 10h.01M6 14h.01M18 14h.01" strokeWidth="2.4" />
      <path d="M9 14h6" />
    </Icon>
  );
}

export function IconBug(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="8" y="7" width="8" height="11" rx="4" />
      <path d="M12 7V4M9 5l-1.5-1.5M15 5l1.5-1.5M4 11h4M16 11h4M4 16h4M16 16h4M9 18v2M15 18v2" />
    </Icon>
  );
}

export function IconFolder(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7z" />
    </Icon>
  );
}

export function IconFullscreenExit(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 3v5H3" />
      <path d="M16 3v5h5" />
      <path d="M21 16h-5v5" />
      <path d="M3 16h5v5" />
    </Icon>
  );
}

// Generic category glyphs for the game/app sidebar — deliberately not
// recreations of any game's actual logo/branding, just a visual category cue.

export function IconGameController(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M6 8h12l2 4.5v5a2 2 0 01-3.4 1.4L14 16h-4l-2.6 2.9A2 2 0 014 17.5v-5L6 8z" />
      <path d="M9 11v3M7.5 12.5h3" />
      <circle cx="16" cy="11.5" r="0.6" fill="currentColor" stroke="none" />
      <circle cx="17.7" cy="13.2" r="0.6" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconGameBlock(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9L12 3z" />
      <path d="M4.5 7.5L12 12l7.5-4.5M12 12v9" />
    </Icon>
  );
}

export function IconGameCrosshair(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="7" />
      <circle cx="12" cy="12" r="1.6" fill="currentColor" stroke="none" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3" />
    </Icon>
  );
}

export function IconGameSword(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M14.5 3.5l6 6-8.5 8.5-6-6z" />
      <path d="M12 10L4 18l-1 3 3-1 8-8" />
      <path d="M16 7l3-3" />
    </Icon>
  );
}

export function IconGameCar(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 16v-3.5L6 8h12l2 4.5V16" />
      <path d="M4 16h16v2a1 1 0 01-1 1h-1.5a1 1 0 01-1-1v-1h-9v1a1 1 0 01-1 1H5a1 1 0 01-1-1v-2z" />
      <circle cx="8" cy="16" r="1.4" fill="currentColor" stroke="none" />
      <circle cx="16" cy="16" r="1.4" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function IconGameRocket(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 2c2.5 2 4 5.5 4 9 0 2-1 4-1 4l-3 1-3-1s-1-2-1-4c0-3.5 1.5-7 4-9z" />
      <circle cx="12" cy="9" r="1.4" fill="currentColor" stroke="none" />
      <path d="M9 15l-2.5 2.5M15 15l2.5 2.5M10 18.5l2 2.5 2-2.5" />
    </Icon>
  );
}

export function IconGameHome(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 11l8-7 8 7" />
      <path d="M6 10v9a1 1 0 001 1h10a1 1 0 001-1v-9" />
      <path d="M10 20v-5h4v5" />
    </Icon>
  );
}

export function IconGameBall(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8" />
      <path d="M12 4v4l3.5 2.5-1.3 4H9.8l-1.3-4L12 8" />
      <path d="M4.5 9.5L8.5 10.5M19.5 9.5l-4 1M8.8 20l1-4M15.2 20l-1-4" />
    </Icon>
  );
}

export function IconGameChat(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 5h16v10H8l-4 4V5z" />
      <path d="M8 9h8M8 12h5" />
    </Icon>
  );
}

export function IconGameGlobe(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8" />
      <path d="M4 12h16M12 4c2.2 2.2 3.3 5 3.3 8s-1.1 5.8-3.3 8c-2.2-2.2-3.3-5-3.3-8S9.8 6.2 12 4z" />
    </Icon>
  );
}

export function IconGameMusic(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="7" cy="17" r="2.4" />
      <circle cx="17" cy="15" r="2.4" />
      <path d="M9.4 17V5.5L19.4 4v11" />
    </Icon>
  );
}
