import { GlobalStyles, ThemeProvider, useMediaQuery } from '@mui/material';
import { alpha, darken, lighten } from '@mui/material/styles';
import { createContext, type ReactNode, useContext, useEffect, useMemo, useState } from 'react';
import { createAdminTheme } from './adminTheme';
import { type Appearance, defaultAppearance, parseAppearance, textScale } from './appearance';

const AppearanceContext = createContext({
  preferences: defaultAppearance,
  update: (_value: Partial<Appearance>) => {},
  reset: () => {},
});
function read(key: string) {
  try {
    return parseAppearance(JSON.parse(localStorage.getItem(key) || 'null'));
  } catch {
    return defaultAppearance;
  }
}
export function AppearanceProvider({
  accountId,
  children,
}: {
  accountId: string;
  children: ReactNode;
}) {
  const key = `arena:appearance:${accountId || 'guest'}`;
  const [preferences, setPreferences] = useState(() => read(key));
  const systemDark = useMediaQuery('(prefers-color-scheme: dark)');
  useEffect(() => {
    setPreferences(read(key));
    const sync = (event: StorageEvent) => {
      if (event.key === key || event.key === null) setPreferences(read(key));
    };
    window.addEventListener('storage', sync);
    return () => window.removeEventListener('storage', sync);
  }, [key]);
  const mode = preferences.mode === 'system' ? (systemDark ? 'dark' : 'light') : preferences.mode;
  const theme = useMemo(() => createAdminTheme(preferences, mode), [preferences, mode]);
  const update = (value: Partial<Appearance>) =>
    setPreferences((current) => {
      const next = parseAppearance({ ...current, ...value });
      try {
        localStorage.setItem(key, JSON.stringify(next));
      } catch {
        /* Continue with an in-memory preference when storage is unavailable. */
      }
      return next;
    });
  return (
    <AppearanceContext value={{ preferences, update, reset: () => update(defaultAppearance) }}>
      <ThemeProvider theme={theme}>
        <GlobalStyles
          styles={{
            ':root': {
              fontSize: `${textScale[preferences.textSize] * 100}%`,
              colorScheme: mode,
              '--workspace-bg': theme.palette.background.default,
              '--workspace-paper': theme.palette.background.paper,
              '--workspace-border': theme.palette.divider,
              '--workspace-text': theme.palette.text.primary,
              '--workspace-muted': theme.palette.text.secondary,
              '--workspace-accent': theme.palette.primary.main,
              '--workspace-accent-soft': alpha(preferences.accent, 0.14),
              '--workspace-sidebar': darken(preferences.accent, 0.78),
              '--workspace-highlight': lighten(preferences.accent, 0.75),
              '--workspace-table': mode === 'dark' ? '#202c38' : '#f8fafb',
            },
          }}
        />
        {children}
      </ThemeProvider>
    </AppearanceContext>
  );
}
export function useAppearance() {
  return useContext(AppearanceContext);
}
