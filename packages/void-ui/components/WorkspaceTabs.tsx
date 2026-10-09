import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface WorkspaceTabDef {
  id: string;
  title: string;
  ariaLabel?: string;
  shortcut?: string;
}

export interface WorkspaceTabsProps {
  workspaces: WorkspaceTabDef[];
  active: string;
  onSelect: (id: string) => void;
  'aria-label'?: string;
}

/**
 * Workspace switcher — ARIA tablist with arrow-key navigation and
 * Home/End. Selecting a tab only changes which workspace is focused;
 * digits 1..3 are handled by the shell (document-level shortcut).
 */
export const WorkspaceTabs: React.FC<WorkspaceTabsProps> = React.memo(
  ({ workspaces, active, onSelect, 'aria-label': ariaLabel = 'Workspaces' }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const tabRefs = React.useRef<Map<string, HTMLButtonElement>>(new Map());
    const activeIndex = Math.max(
      0,
      workspaces.findIndex((w) => w.id === active),
    );

    const move = (delta: number) => {
      const next =
        (activeIndex + delta + workspaces.length) % workspaces.length;
      const id = workspaces[next].id;
      onSelect(id);
      tabRefs.current.get(id)?.focus();
    };

    return (
      <div
        role="tablist"
        aria-label={ariaLabel}
        aria-orientation="horizontal"
        style={{ display: 'flex', gap: 4 }}
      >
        {workspaces.map((w) => {
          const isActive = w.id === active;
          return (
            <button
              key={w.id}
              ref={(el) => {
                if (el) tabRefs.current.set(w.id, el);
                else tabRefs.current.delete(w.id);
              }}
              type="button"
              role="tab"
              id={`workspace-tab-${w.id}`}
              aria-selected={isActive}
              aria-label={w.ariaLabel ?? `${w.title} workspace`}
              aria-controls={`workspace-panel-${w.id}`}
              tabIndex={isActive ? 0 : -1}
              className={`${focusClass} ${animatedClass}`}
              onClick={() => onSelect(w.id)}
              onKeyDown={(e) => {
                if (e.key === 'ArrowRight') {
                  e.preventDefault();
                  move(1);
                } else if (e.key === 'ArrowLeft') {
                  e.preventDefault();
                  move(-1);
                } else if (e.key === 'Home') {
                  e.preventDefault();
                  onSelect(workspaces[0].id);
                  tabRefs.current.get(workspaces[0].id)?.focus();
                } else if (e.key === 'End') {
                  e.preventDefault();
                  const last = workspaces[workspaces.length - 1].id;
                  onSelect(last);
                  tabRefs.current.get(last)?.focus();
                }
              }}
              style={{
                fontFamily: tokens.mono,
                fontSize: 12,
                padding: '6px 14px',
                borderRadius: `${tokens.radius} ${tokens.radius} 0 0`,
                border: `1px solid ${isActive ? tokens.accent : tokens.border}`,
                borderBottom: isActive ? `1px solid ${tokens.bg}` : `1px solid ${tokens.border}`,
                background: isActive ? tokens.surfaceRaised : 'transparent',
                color: isActive ? tokens.text : tokens.textSecondary,
                cursor: 'pointer',
              }}
            >
              {w.title}
              {w.shortcut ? (
                <span
                  aria-hidden
                  style={{ marginLeft: 6, fontSize: 9, color: tokens.textMuted }}
                >
                  {w.shortcut}
                </span>
              ) : null}
            </button>
          );
        })}
      </div>
    );
  },
);
WorkspaceTabs.displayName = 'WorkspaceTabs';
