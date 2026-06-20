import { configureDefaultHttpClient } from '@gaming-cafe/utils';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import App from './App';
import { handleAuthExpired } from './lib/authSession';
import '@gaming-cafe/theme/tokens.css';
import './globals.css';

configureDefaultHttpClient({
  onUnauthorized: (context) => {
    void handleAuthExpired(context);
  },
});

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Failed to find the root element');

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
);
