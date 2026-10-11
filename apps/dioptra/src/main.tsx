import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import './styles/tokens.css';
import './ui/ui.css';
import './styles/app.css';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import type { Changed, Command } from './engine/types';
import { withEngine } from './store/engine';
import { applyChanged } from './store/sync';

// A small hook for end-to-end tests and debugging: send a command, then refresh the UI from
// what it changed (exactly as a menu command would).
declare global {
  interface Window {
    __dioptra?: {
      dispatch: (c: Command) => Promise<unknown>;
      applyChanged: (c: Changed) => Promise<void>;
      /** Run a command of the registry by id (the macOS native menu, tests). */
      runCommand?: (id: string) => Promise<boolean>;
    };
  }
}
window.__dioptra = { dispatch: (c) => withEngine((e) => e.dispatch(c)), applyChanged };

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
