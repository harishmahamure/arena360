import React from 'react';
import { createRoot } from 'react-dom/client';
import Portal from './Portal';
import './portal.css';
const root = document.getElementById('root');
if (!root) throw new Error('Portal root is missing');
createRoot(root).render(
  <React.StrictMode>
    <Portal />
  </React.StrictMode>,
);
