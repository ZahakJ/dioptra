// The application shell: top bar, project rail, work area, inspector, status line, and the
// overlays (palette, dialogs, context menu, tooltips, notices).
import { useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { TableView } from '../grid/TableView';
import { registerFeatures } from '../features';
import { bootEngine } from '../store/engine';
import { useLab } from '../store/lab';
import { useProject } from '../store/project';
import { useEngineStatus } from '../store/status';
import { INSPECTOR_MAX, INSPECTOR_MIN, RAIL_MAX, RAIL_MIN, resolveTheme, useUi } from '../store/ui';
import { ContextMenuHost } from './ContextMenu';
import { DialogHost } from './dialogs';
import { LabDialog } from './LabDialog';
import { startRecords } from './Records';
import { Inspector } from './inspector';
import { registerCoreInspectorSections } from './inspectorSections';
import { TooltipHost } from '../ui';
import { commands } from './commands';
import { installNativeMenu } from './nativeMenu';
import { Notices } from './Notices';
import { Palette } from './Palette';
import { ProjectRail } from './ProjectRail';
import { Resizer } from './Resizer';
import { ShortcutsDialog } from './ShortcutsDialog';
import { StatusLine } from './StatusLine';
import { TopBar } from './TopBar';
import { useShortcuts } from './useShortcuts';
import { registerWorkArea, WorkArea } from './workArea';

let registered = false;
/** Core registrations; Phase 5B adds its own after this runs (see app/extensions.ts). */
export function registerCore(): void {
  if (registered) return;
  registered = true;
  registerWorkArea('table', TableView);
  registerCoreInspectorSections();
  registerFeatures();
}

function useTheme(): void {
  const pref = useUi((s) => s.theme);
  const backend = useEngineStatus((s) => s.backend);
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
  // The window frame follows: null hands the title bar back to the OS theme.
  useEffect(() => {
    if (backend !== 'tauri') return;
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(pref === 'system' ? null : pref))
      .catch(() => undefined);
  }, [pref, backend]);
}

function useNativeMenu(): void {
  const backend = useEngineStatus((s) => s.backend);
  useEffect(() => {
    if (backend === 'tauri') void installNativeMenu();
  }, [backend]);
  useEffect(() => {
    if (window.__dioptra) window.__dioptra.runCommand = (id: string) => commands.run(id);
  }, []);
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
  useNativeMenu();
  const status = useEngineStatus();
  const ui = useUi(
    useShallow((s) => ({
      railVisible: s.railVisible,
      railWidth: s.railWidth,
      inspectorVisible: s.inspectorVisible,
      inspectorWidth: s.inspectorWidth,
      setRailWidth: s.setRailWidth,
      setInspectorWidth: s.setInspectorWidth,
    })),
  );

  useEffect(() => {
    startRecords();
    bootEngine()
      .then(() => useLab.getState().load().catch(() => undefined))
      .then(() => useProject.getState().refresh())
      .then((view) => {
        const first = view?.items[0];
        if (first) useProject.getState().open(first.id);
      })
      .catch(() => undefined);
  }, []);

  return (
    <div className="app" data-testid="app" data-ready={status.ready ? 'true' : 'false'}>
      <TopBar />
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
      <LabDialog />
      <DialogHost />
      <ContextMenuHost />
      <Notices />
      <TooltipHost />
    </div>
  );
}
