import { ArrowBack, Fullscreen, FullscreenExit } from '@mui/icons-material';
import { Box, Button, IconButton, Tooltip, Typography } from '@mui/material';
import { type ReactNode, useEffect, useState } from 'react';
import { Link as RouterLink } from 'react-router-dom';

export interface CounterSaleLayoutProps {
  title: string;
  exitTo: string;
  catalog: ReactNode;
  summary: ReactNode;
  alerts?: ReactNode;
  actions: ReactNode;
  busy: boolean;
}

function useBrowserFullscreen() {
  const supported = typeof document !== 'undefined' && document.fullscreenEnabled;
  const [active, setActive] = useState(() => !!document.fullscreenElement);
  useEffect(() => {
    const sync = () => setActive(!!document.fullscreenElement);
    document.addEventListener('fullscreenchange', sync);
    return () => document.removeEventListener('fullscreenchange', sync);
  }, []);
  const toggle = () =>
    void (
      document.fullscreenElement
        ? document.exitFullscreen()
        : document.documentElement.requestFullscreen()
    ).catch(() => {
      /* The browser may refuse; the screen still works without it. */
    });
  return { supported, active, toggle };
}

/**
 * Full-screen counter sale: the shell chrome is hidden on these routes, so this slim bar
 * is the only header. Pick on the left, pay on the right; submit handlers validate.
 */
export function CounterSaleLayout({
  title,
  exitTo,
  catalog,
  summary,
  alerts,
  actions,
  busy,
}: CounterSaleLayoutProps) {
  const fullscreen = useBrowserFullscreen();
  return (
    <Box sx={{ minHeight: '100dvh', bgcolor: 'background.default' }}>
      <Box
        component="header"
        sx={{
          position: 'sticky',
          top: 0,
          zIndex: 1100,
          display: 'flex',
          alignItems: 'center',
          gap: 1,
          px: { xs: 1, md: 2 },
          py: 0.75,
          bgcolor: 'background.paper',
          borderBottom: 1,
          borderColor: 'divider',
        }}
      >
        <Button component={RouterLink} to={exitTo} startIcon={<ArrowBack />}>
          Exit
        </Button>
        <Typography component="h1" variant="h6" noWrap sx={{ flex: 1 }}>
          {title}
        </Typography>
        {fullscreen.supported && (
          <Tooltip title={fullscreen.active ? 'Leave full screen' : 'Full screen'}>
            <IconButton
              onClick={fullscreen.toggle}
              aria-label={fullscreen.active ? 'Leave full screen' : 'Enter full screen'}
            >
              {fullscreen.active ? <FullscreenExit /> : <Fullscreen />}
            </IconButton>
          </Tooltip>
        )}
      </Box>
      <Box sx={{ px: { xs: 1.5, md: 2.5 }, py: 2 }}>
        {alerts}
        <Box
          component="fieldset"
          disabled={busy}
          sx={{
            m: 0,
            p: 0,
            border: 0,
            minWidth: 0,
            display: 'grid',
            gap: 2,
            gridTemplateColumns: { xs: '1fr', md: 'minmax(0, 7fr) minmax(340px, 5fr)' },
            alignItems: 'start',
          }}
        >
          <Box sx={{ minWidth: 0 }}>{catalog}</Box>
          <Box sx={{ minWidth: 0, position: { md: 'sticky' }, top: { md: 72 } }}>
            {summary}
            <Box sx={{ mt: 2 }}>{actions}</Box>
          </Box>
        </Box>
      </Box>
    </Box>
  );
}
