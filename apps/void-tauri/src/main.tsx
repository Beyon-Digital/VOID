import React from 'react';
import { createRoot } from 'react-dom/client';
import { StudioShell } from './ui/shell';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <StudioShell />
  </React.StrictMode>,
);
