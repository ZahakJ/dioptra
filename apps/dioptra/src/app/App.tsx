// The application shell: menu bar, project rail, work area, inspector, status line, and the
// overlays (palette, dialogs, context menu, notices).
import { useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { TableView } from '../grid/TableView';
import { bootEngine } from '../store/engine';
import { useProject } from '../store/project';
import { useEngineStatus } from '../store/status';
import { INSPECTOR_MAX, INSPECTOR_MIN, RAIL_MAX, RAIL_MIN, resolveTheme, useUi } from '../store/ui';
import { ContextMenuHost } from './ContextMenu';
import { DialogHost } from './dialogs';
import { Inspector } from './inspector';
import { registerCoreInspectorSections } from './inspectorSections';
import { MenuBar } from './MenuBar';
import { Notices } from './Notices';
import { Palette } from './Palette';
import { ProjectRail } from './ProjectRail';
import { Resizer } from './Resizer';
import { ShortcutsDialog } from './ShortcutsDialog';
import { StatusLine } from './StatusLine';
import { useShortcuts } from './useShortcuts';
import { registerWorkArea, WorkArea } from './workArea';

let registered = false;
/** Core registrations; Phase 5B adds its own after this runs (see app/extensions.ts). */
export function registerCore(): void {
  if (registered) return;
  registered = true;
  registerWorkArea('table', TableView);
  registerCoreInspectorSections();
}

function useTheme(): void {
  const pref = useUi((s) => s.theme);
  const [systemDark, setSystemDark] = useState(
    () => typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-color-scheme: dark)').matches,
  );
  useEffect(() => {
    const mq = window.matchMedia?.('(prefers-color-scheme: dark)');
    if (!mq) return;
    const on = () => setSystemDark(mq.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = resolveTheme(pref, systemDark);
    root.dataset.themePref = pref;
    root.style.colorScheme = root.dataset.theme;
  }, [pref, systemDark]);
}

function useWindowTitle(): void {
  const title = useProject((s) => s.view?.title);
  const dirty = useProject((s) => s.view?.dirty);
  const backend = useEngineStatus((s) => s.backend);
  useEffect(() => {
    const t = `Dioptra — ${title ?? 'Untitled'}`;
    document.title = t;
    if (backend === 'tauri') {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => getCurrentWindow().setTitle(dirty ? `${t} (edited)` : t))
        .catch(() => undefined);
    }
  }, [title, dirty, backend]);
  useEffect(() => {
    const onBefore = (e: BeforeUnloadEvent) => {
      const v = useProject.getState().view;
      if (v?.dirty && v.items.length > 0 && backend !== 'tauri') e.preventDefault();
    };
    window.addEventListener('beforeunload', onBefore);
    return () => window.removeEventListener('beforeunload', onBefore);
  }, [backend]);
}

export function App() {
  registerCore();
  useTheme();
  useWindowTitle();
  useShortcuts();
  const status = useEngineStatus();
  const ui = useUi(
    useShallow((s) => ({
      railVisible: s.railVisible,
      railWidth: s.railWidth,
      inspectorVisible: s.inspectorVisible,
      inspectorWidth: s.inspectorWidth,
      setRailWidth: s.setRailWidth,
      setInspectorWidth: s.setInspectorWidth,
      setPaletteOpen: s.setPaletteOpen,
    })),
  );

  useEffect(() => {
    bootEngine()
      .then(() => useProject.getState().refresh())
      .then((view) => {
        const first = view?.items[0];
        if (first) useProject.getState().open(first.id);
      })
      .catch(() => undefined);
  }, []);

  return (
    <div className="app" data-testid="app" data-ready={status.ready ? 'true' : 'false'}>
      <header className="titlebar">
        <span className="brand" aria-label="Dioptra">
          <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" className="brand-mark">
            <circle cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.25" />
            <path d="M7 0.5v4M7 9.5v4M0.5 7h4M9.5 7h4" stroke="currentColor" strokeWidth="1.25" />
          </svg>
          Dioptra
        </span>
        <MenuBar />
        <button
          type="button"
          className="palette-hint"
          onClick={() => ui.setPaletteOpen(true)}
          data-testid="open-palette"
          title="Command palette"
        >
          Search commands <kbd className="menu-kbd">Ctrl+K</kbd>
        </button>
      </header>
      {!status.ready ? (
        <div className="boot" data-testid="engine-loading">
          {status.error ? `The engine failed to load: ${status.error}` : 'Loading the engine...'}
        </div>
      ) : (
        <div className="app-main">
          {ui.railVisible && (
            <>
              <div className="rail-wrap" style={{ width: ui.railWidth }}>
                <ProjectRail />
              </div>
              <Resizer value={ui.railWidth} onChange={ui.setRailWidth} side="left" label="Resize project rail" min={RAIL_MIN} max={RAIL_MAX} />
            </>
          )}
          <WorkArea />
          {ui.inspectorVisible && (
            <>
              <Resizer
                value={ui.inspectorWidth}
                onChange={ui.setInspectorWidth}
                side="right"
                label="Resize inspector"
                min={INSPECTOR_MIN}
                max={INSPECTOR_MAX}
              />
              <div className="inspector-wrap" style={{ width: ui.inspectorWidth }}>
                <Inspector />
              </div>
            </>
          )}
        </div>
      )}
      <StatusLine />
      <Palette />
      <ShortcutsDialog />
      <DialogHost />
      <ContextMenuHost />
      <Notices />
    </div>
  );
}
