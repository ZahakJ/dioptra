import React from 'react';
import { createRoot } from 'react-dom/client';

function App() {
  return <main>Dioptra</main>;
}

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
