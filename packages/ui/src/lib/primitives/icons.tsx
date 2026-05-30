export type FallbackIconKind = 'browser' | 'game';

const BROWSER_NAME_HINTS = [
  'chrome',
  'edge',
  'firefox',
  'browser',
  'opera',
  'brave',
  'safari',
  'internet',
];

export function defaultIconKind(name: string): FallbackIconKind {
  const n = name.toLowerCase();
  return BROWSER_NAME_HINTS.some((hint) => n.includes(hint)) ? 'browser' : 'game';
}

export interface SvgIconProps {
  size?: number;
  className?: string;
  style?: React.CSSProperties;
  title?: string;
}

function commonProps({ size = 24, className, style, title }: SvgIconProps) {
  return {
    viewBox: '0 0 24 24',
    width: size,
    height: size,
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 1.6,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    className,
    style,
    role: 'img',
    'aria-label': title,
  };
}

export function BrowserGlyph(props: SvgIconProps) {
  const title = props.title ?? 'Browser';
  return (
    <svg {...commonProps({ ...props, title })}>
      <title>{title}</title>
      <circle cx="12" cy="12" r="9" />
      <ellipse cx="12" cy="12" rx="4" ry="9" />
      <path d="M3 9h18M3 15h18" />
    </svg>
  );
}

export function GamepadGlyph(props: SvgIconProps) {
  const title = props.title ?? 'Game';
  return (
    <svg {...commonProps({ ...props, title })}>
      <title>{title}</title>
      <path d="M6 12h4M8 10v4" />
      <circle cx="15.5" cy="11.5" r="1" />
      <circle cx="17.5" cy="13.5" r="1" />
      <path d="M17.5 6h-11A4.5 4.5 0 0 0 2 10.5l1 6A2.5 2.5 0 0 0 7.4 17l1-1.5h7.2l1 1.5a2.5 2.5 0 0 0 4.4-.5l1-6A4.5 4.5 0 0 0 17.5 6Z" />
    </svg>
  );
}

export function SpeakerGlyph({ muted, ...props }: SvgIconProps & { muted?: boolean }) {
  const title = props.title ?? 'Volume';
  return (
    <svg {...commonProps({ ...props, title })}>
      <title>{title}</title>
      <path d="M4 9v6h4l5 4V5L8 9H4z" />
      {muted ? (
        <path d="M16 9l5 5M21 9l-5 5" />
      ) : (
        <path d="M16 8.5a4 4 0 0 1 0 7M18.5 6a7 7 0 0 1 0 12" />
      )}
    </svg>
  );
}

export function SettingsGlyph(props: SvgIconProps) {
  const title = props.title ?? 'Settings';
  return (
    <svg {...commonProps({ ...props, title })}>
      <title>{title}</title>
      <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7z" />
      <path d="M19.4 15a1.8 1.8 0 0 0 .36 1.98l.05.05a2.18 2.18 0 1 1-3.08 3.08l-.05-.05A1.8 1.8 0 0 0 14.7 19.7a1.8 1.8 0 0 0-1.1 1.65V21.5a2.18 2.18 0 1 1-4.36 0v-.08A1.8 1.8 0 0 0 8.1 19.78a1.8 1.8 0 0 0-1.98.36l-.05.05a2.18 2.18 0 1 1-3.08-3.08l.05-.05A1.8 1.8 0 0 0 3.4 15.1a1.8 1.8 0 0 0-1.65-1.1H1.6a2.18 2.18 0 1 1 0-4.36h.08A1.8 1.8 0 0 0 3.32 8.5a1.8 1.8 0 0 0-.36-1.98l-.05-.05a2.18 2.18 0 1 1 3.08-3.08l.05.05A1.8 1.8 0 0 0 8 3.8a1.8 1.8 0 0 0 1.1-1.65V2a2.18 2.18 0 1 1 4.36 0v.08A1.8 1.8 0 0 0 14.6 3.72a1.8 1.8 0 0 0 1.98-.36l.05-.05a2.18 2.18 0 1 1 3.08 3.08l-.05.05A1.8 1.8 0 0 0 19.3 8.4a1.8 1.8 0 0 0 1.65 1.1h.15a2.18 2.18 0 1 1 0 4.36h-.08A1.8 1.8 0 0 0 19.4 15z" />
    </svg>
  );
}
