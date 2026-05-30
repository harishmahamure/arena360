/**
 * MUI Theme Utility Classes
 * Use these for type-safe className usage
 */

export const themeClasses = {
  // Shared shell / surfaces
  authShell: 'gz-auth-shell',
  authCard: 'gz-auth-card',
  authContent: 'gz-auth-content',
  authScrim: 'gz-auth-scrim',
  authFooter: 'gz-auth-footer',
  glassPanel: 'gz-glass-panel',

  // Shared controls
  pillButton: 'gz-pill-button',
  iconButton: 'gz-icon-button',
  popover: 'gz-popover',
  audioControl: 'gz-audio-control',
  profilePill: 'gz-profile-pill',

  // Hover effects
  hoverLift: 'hover-lift',
  hoverScale: 'hover-scale',

  // Animations
  animateFadeIn: 'animate-fadeIn',
  animateFadeInUp: 'animate-fadeInUp',
  animateFadeInDown: 'animate-fadeInDown',

  // Responsive layout
  responsiveContainer: 'responsive-container',
  responsiveGrid: 'responsive-grid',
  responsiveGrid2: 'responsive-grid-2',
  responsiveGrid3: 'responsive-grid-3',
  responsiveGrid4: 'responsive-grid-4',
  responsiveFlex: 'responsive-flex',

  // Visibility
  hideOnMobile: 'hide-on-mobile',
  hideOnTablet: 'hide-on-tablet',
  showOnMobile: 'show-on-mobile',
  showOnTablet: 'show-on-tablet',
} as const;

export type ThemeClass = (typeof themeClasses)[keyof typeof themeClasses];

/**
 * Combines multiple class names, filtering out falsy values
 */
export function cn(...classes: (ThemeClass | string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(' ');
}

export default themeClasses;
