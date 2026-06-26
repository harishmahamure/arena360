import '@gaming-cafe/theme/tokens.css';
import './app/app.css';
import './app/arena360.css';

import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './app/App';
import { ErrorBoundary } from './components/ErrorBoundary';
import { initBootDiagnostics, reportBootReady, verifyStylesLoaded } from './lib/bootDiagnostics';

void initBootDiagnostics();

const rootEl = document.getElementById('root');
if (!rootEl) {
  throw new Error('Missing #root element');
}

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);

verifyStylesLoaded();
void reportBootReady();
