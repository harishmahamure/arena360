export interface AuthShellProps {
  children: React.ReactNode;
  /** Product/brand name, used in the default footer. */
  title?: string;
  /** Looped, muted background video URL. */
  videoUrl?: string;
  /** Optional rendered background node (kiosk can provide catalog art/video). */
  backgroundSlot?: React.ReactNode;
  /** Optional brand slot rendered above the card (logo, station name). */
  brandSlot?: React.ReactNode;
  /** Optional footer override; defaults to a copyright line. */
  footer?: React.ReactNode;
  className?: string;
  cardClassName?: string;
}

export const DEFAULT_AUTH_VIDEO = 'https://assets.designtemplate.io/trailer_dualshock.webm';

/**
 * Framework-neutral auth shell shared by admin and kiosk. Visuals are provided
 * by the `gz-auth-*` classes in `@gaming-cafe/theme/tokens.css`.
 */
export function AuthShell({
  children,
  title = 'Arena360',
  videoUrl = DEFAULT_AUTH_VIDEO,
  backgroundSlot,
  brandSlot,
  footer,
  className,
  cardClassName,
}: AuthShellProps) {
  return (
    <section className={['gz-auth-shell', className].filter(Boolean).join(' ')}>
      {backgroundSlot ?? (
        <video className="gz-auth-bg-media" autoPlay loop muted playsInline>
          <source src={videoUrl} type="video/mp4" />
          <source src={videoUrl} type="video/webm" />
        </video>
      )}
      <div className="gz-auth-scrim" />
      <div className="gz-auth-content">
        {brandSlot ? <div className="gz-auth-brand-slot">{brandSlot}</div> : null}
        <div className={['gz-auth-card', cardClassName].filter(Boolean).join(' ')}>{children}</div>
        {footer ?? (
          <p className="gz-auth-footer">
            © {new Date().getFullYear()} {title}. All rights reserved.
          </p>
        )}
      </div>
    </section>
  );
}
