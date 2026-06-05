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

export function IconPencil(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 013 3L7 19l-4 1 1-4 12.5-12.5z" />
    </Icon>
  );
}
