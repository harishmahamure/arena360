'use client';

import { AuthShell, DEFAULT_AUTH_VIDEO } from '../primitives';

export interface AuthLayoutProps {
  children: React.ReactNode;
  /** Product/brand name, used in the default footer. */
  title?: string;
  description?: string;
  features?: string[];
  /** Looped, muted background video URL. */
  videoUrl?: string;
  /** Optional brand slot rendered above the card (logo, station name). */
  brandSlot?: React.ReactNode;
  /** Optional footer override; defaults to a copyright line. */
  footer?: React.ReactNode;
}

/**
 * Dark, ggCircuit-style authentication shell: a looped video background with a
 * frosted slate glass card centered on top. Shared by the admin login and the
 * kiosk player login so both surfaces match.
 */
export default function AuthLayout({
  children,
  title = 'Arena360',
  videoUrl = DEFAULT_AUTH_VIDEO,
  brandSlot,
  footer,
}: AuthLayoutProps) {
  return (
    <AuthShell title={title} videoUrl={videoUrl} brandSlot={brandSlot} footer={footer}>
      {children}
    </AuthShell>
  );
}
