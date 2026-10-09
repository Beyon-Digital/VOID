import React from 'react';
import { createRoot } from 'react-dom/client';
import { SignalStudioShell } from './ui/studio/StudioShell';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <SignalStudioShell />
  </React.StrictMode>,
);
