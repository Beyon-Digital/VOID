import * as React from 'react';
import {
  IconButton,
  StatusBadge,
  WorkspaceTab,
  tokens,
  useVoidTheme,
} from 'void-ui';
import {
  WORKSPACES,
  WORKSPACE_ORDER,
  isWorkspaceId,
  type WorkspaceId,
} from 'void-studio';
import { useProjectSummary } from './useStudioData';
import { useStudio } from 'void-studio';
import type { SaveResultEvent } from 'void-client';

function savedStateText(save: SaveResultEvent | undefined): {
  text: string;
  status: 'saved' | 'error' | null;
} {
  if (!save) return { text: '', status: null };
  if (save.status === 'SAVE_DURABLE') return { text: 'Saved locally', status: 'saved' };
  if (save.status === 'SAVE_FAILED') return { text: 'Save failed', status: 'error' };
  return { text: '', status: null };
}

export interface StudioHeaderProps {
  /** Active screen id (workspace id or affordance screen). */
  activeScreen: string;
  onNavigate: (screenId: string) => void;
  onToggleDevSurface: () => void;
  devSurfaceOpen: boolean;
}

/**
 * Signal Studio header (S01): project name + truthful saved state, the five
 * workspace tabs, and the library/jobs/setup/export affordances. Dev-surface
 * and theme toggles live at the right edge.
 */
export const StudioHeader: React.FC<StudioHeaderProps> = ({
  activeScreen,
  onNavigate,
  onToggleDevSurface,
  devSurfaceOpen,
}) => {
  const summary = useProjectSummary();
  const save = useStudio((s) => s.telemetry.save);
  const projectId = useStudio((s) => s.projectId);
  const { theme, toggleTheme } = useVoidTheme();
  const saved = savedStateText(save);
  const activeWorkspace = isWorkspaceId(activeScreen) ? (activeScreen as WorkspaceId) : null;

  return (
    <header
      role="banner"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: tokens.space16,
        height: 52,
        padding: `0 ${tokens.space16}`,
        borderBottom: `1px solid ${tokens.line}`,
        background: tokens.surface,
        flexShrink: 0,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'baseline', gap: tokens.space8, minWidth: 0 }}>
        <span
          className="void-type-title"
          style={{
            color: tokens.text,
            maxWidth: 260,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {summary.name ?? 'VOID Studio'}
        </span>
        {saved.status ? (
          <StatusBadge status={saved.status === 'saved' ? 'saved' : 'error'} label={saved.text} hideDot />
        ) : projectId ? null : (
          <span className="void-type-small" style={{ color: tokens.subtle }}>
            no project open
          </span>
        )}
      </div>

      <nav
        role="tablist"
        aria-label="Workspaces"
        style={{ display: 'flex', alignItems: 'stretch', gap: 2, alignSelf: 'stretch' }}
      >
        {WORKSPACE_ORDER.map((id) => (
          <WorkspaceTab
            key={id}
            tabId={id}
            title={WORKSPACES[id].title}
            shortcut={`⌃${WORKSPACES[id].shortcut}`}
            active={activeWorkspace === id}
            onClick={() => onNavigate(id)}
            style={{ alignSelf: 'center' }}
          />
        ))}
      </nav>

      <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: tokens.space4 }}>
        <IconButton
          icon="▤"
          aria-label="Open library"
          pressed={activeScreen === 'library'}
          onClick={() => onNavigate('library')}
        />
        <IconButton
          icon="⟳"
          aria-label="Open jobs"
          pressed={activeScreen === 'jobs'}
          onClick={() => onNavigate('jobs')}
        />
        <IconButton
          icon="⚙"
          aria-label="Open setup"
          pressed={activeScreen === 'setup'}
          onClick={() => onNavigate('setup')}
        />
        <IconButton
          icon="⤒"
          aria-label="Open export"
          pressed={activeScreen === 'export'}
          onClick={() => onNavigate('export')}
        />
        <span
          aria-hidden
          style={{ width: 1, height: 20, background: tokens.line, margin: `0 ${tokens.space4}` }}
        />
        <IconButton
          icon={theme === 'dark' ? '☾' : '☀'}
          aria-label={`Switch to ${theme === 'dark' ? 'daylight' : 'dark'} theme`}
          onClick={toggleTheme}
        />
        <IconButton
          icon="⌘"
          aria-label="Toggle developer surface"
          pressed={devSurfaceOpen}
          onClick={onToggleDevSurface}
        />
      </div>
    </header>
  );
};
