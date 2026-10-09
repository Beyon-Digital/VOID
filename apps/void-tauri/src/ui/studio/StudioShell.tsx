// Signal Studio shell — the default route for the VOID desktop app.
//
// Chrome: StudioHeader (5 workspace tabs + library/jobs/setup/export +
// theme + dev-surface toggle), TransportBar, workspace screen region
// (file-convention registry — see registry.ts), HealthBar.
//
// Contracts honored here:
//   - Escape: the shared escape stack (void-ui editing) lets an active
//     gesture/preview consume Escape at capture time BEFORE any chrome
//     or drawer reacts.
//   - Keyboard/IME: global shortcuts never fire while a text-editable
//     target or IME composition owns the event.
//   - Compact (≤1280px): screens collapse secondary columns to drawers;
//     transport stays mounted.
//   - Honest unavailability: unimplemented screen ids render a placeholder
//     that names the id — never fake content.

import * as React from 'react';
import {
  ActionButton,
  VoidThemeProvider,
  globalEscapeStack,
  injectVoidStyles,
  mayUseGlobalShortcut,
  tokens,
  useToasts,
  ToastViewport,
} from 'void-ui';
import {
  assertEditorViewState,
  editorStore,
  isWorkspaceId,
  useStudio,
  workspaceForShortcut,
} from 'void-studio';
import { ensureClientStarted } from '../client';
import { StudioShell as DeveloperSurface } from '../shell';
import { LauncherPanel } from '../support';
import { StudioHeader } from './StudioHeader';
import { TransportBar } from './TransportBar';
import { HealthBar } from './HealthBar';
import { ScreenPlaceholder } from './ScreenPlaceholder';
import { screenFor } from './registry';
import {
  StudioCompactProvider,
  useHashRoute,
  useStudioCompact,
} from './useStudioData';

/** Routes that render without an open project (S13 launcher flow + S15 setup). */
const PROJECT_FREE_SCREENS = new Set(['projects', 'new-project', 'setup', 'dev-gallery']);

export function SignalStudioShell() {
  const [route, navigateRaw] = useHashRoute('arrange');
  const compact = useStudioCompact();
  const [devOpen, setDevOpen] = React.useState(false);
  const projectId = useStudio((s) => s.projectId);
  const { toasts, dismiss } = useToasts();

  React.useEffect(() => {
    injectVoidStyles();
    void ensureClientStarted();
  }, []);

  // Dev-time sanity: the editor store must stay view-state-only.
  React.useEffect(() => editorStore.subscribe((s) => assertEditorViewState(s)), []);

  const navigate = React.useCallback(
    (id: string) => {
      navigateRaw(id);
      if (isWorkspaceId(id)) editorStore.getState().actions.setWorkspace(id);
    },
    [navigateRaw],
  );

  // Global keys — capture phase so gesture-Escape wins before chrome/drawers.
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (globalEscapeStack.consumeEscape()) {
          e.preventDefault();
          e.stopPropagation();
        }
        return;
      }
      if (!mayUseGlobalShortcut({ target: e.target as HTMLElement, nativeIsComposing: e.isComposing })) {
        return;
      }
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
        const ws = workspaceForShortcut(e.key);
        if (ws) {
          e.preventDefault();
          navigate(ws);
        }
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [navigate]);

  if (devOpen) {
    return (
      <VoidThemeProvider style={{ minHeight: '100vh', display: 'flex', flexDirection: 'column' }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: tokens.space12,
            padding: `${tokens.space4} ${tokens.space16}`,
            borderBottom: `1px solid ${tokens.line}`,
            background: tokens.surface,
          }}
        >
          <span className="void-type-small" style={{ color: tokens.subtle }}>
            Developer surface — engine/dashboard/support tooling
          </span>
          <ActionButton size="sm" variant="secondary" onClick={() => setDevOpen(false)}>
            Back to studio
          </ActionButton>
        </div>
        <DeveloperSurface />
      </VoidThemeProvider>
    );
  }

  const screenId = route.replace(/\//g, '-');
  const Screen = screenFor(screenId);
  // Screens that work without an open project (S13/S14/S15); anything else
  // falls back to the projects screen (S13) — the LauncherPanel stays as the
  // fallback while the registry hasn't discovered one.
  const HomeScreen = screenFor('projects');
  const needsProject = !projectId && !PROJECT_FREE_SCREENS.has(screenId);

  return (
    <VoidThemeProvider
      style={{
        height: '100vh',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <StudioCompactProvider value={compact}>
        <StudioHeader
          activeScreen={screenId}
          onNavigate={navigate}
          onToggleDevSurface={() => setDevOpen(true)}
          devSurfaceOpen={devOpen}
        />
        <TransportBar onOpenExport={() => navigate('export')} />
        <main
          role="tabpanel"
          id={`workspace-panel-${screenId}`}
          aria-labelledby={isWorkspaceId(screenId) ? `workspace-tab-${screenId}` : undefined}
          style={{ display: 'flex', flex: 1, minHeight: 0, overflow: 'hidden' }}
        >
          {needsProject ? (
            HomeScreen ? (
              <HomeScreen />
            ) : (
              <LauncherPanel />
            )
          ) : Screen ? (
            <Screen />
          ) : (
            <ScreenPlaceholder screenId={screenId} />
          )}
        </main>
        <HealthBar />
        <ToastViewport toasts={toasts} onDismiss={dismiss} />
      </StudioCompactProvider>
    </VoidThemeProvider>
  );
}

export default SignalStudioShell;
