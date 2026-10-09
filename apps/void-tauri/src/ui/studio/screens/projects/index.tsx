// S13 Projects / start here (Figma 4:1208).
//
// Real data: the recents store (`void-studio/src/recents` — reopen tokens
// recorded only after APPLIED/DUPLICATE receipts) plus engine attach state.
// "Open" runs the real coordinator flow (attach → OpenProjectOp). No seeded
// rows; an empty recents store renders the honest empty state.

import * as React from 'react';
import { ActionButton, Field, RecentProjectRow, StatusBadge, TextInput, tokens } from 'void-ui';
import { useStudio, useStore } from 'void-studio';
import { recents } from '../../../support';
import { openProjectByDir } from './lifecycle';

function navigate(id: string) {
  window.location.hash = `/${id}`;
}

const navItem: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: tokens.space8,
  padding: `${tokens.space8} ${tokens.space12}`,
  borderRadius: tokens.radius8,
  color: tokens.muted,
  fontSize: 13,
  cursor: 'pointer',
  background: 'none',
  border: 'none',
  textAlign: 'left',
  width: '100%',
};

const sectionLabel: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 500,
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: tokens.subtle,
  margin: '0 0 8px',
};

export default function ProjectsScreen() {
  const attached = useStudio((s) => s.engine.attached);
  const projectId = useStudio((s) => s.projectId);
  const entries = useStore(recents(), (s) => s.entries);
  const persistError = useStore(recents(), (s) => s.persistError);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [error, setError] = React.useState('');
  const [openDir, setOpenDir] = React.useState('');

  const openRecent = async (containerDir: string, pid: string, name?: string) => {
    setBusy(containerDir);
    setError('');
    const r = await openProjectByDir({ containerDir, projectId: pid, displayName: name });
    if (!r.ok) setError(r.error ?? 'open failed');
    else navigate('arrange');
    setBusy(null);
  };

  const openByPath = async () => {
    const dir = openDir.trim();
    if (!dir) return;
    setBusy('path');
    setError('');
    const r = await openProjectByDir({ containerDir: dir });
    if (!r.ok) setError(r.error ?? 'open failed');
    else navigate('arrange');
    setBusy(null);
  };

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
      {/* Left rail — library navigation (semantic routes, honest placeholders). */}
      <nav
        aria-label="Library"
        style={{
          width: 220,
          flexShrink: 0,
          display: 'flex',
          flexDirection: 'column',
          gap: 2,
          padding: tokens.space16,
          borderRight: `1px solid ${tokens.line}`,
          background: tokens.surface,
          overflowY: 'auto',
        }}
      >
        <p style={sectionLabel}>Library</p>
        <button type="button" style={{ ...navItem, color: tokens.text, background: tokens.raised }} aria-current="page">
          Recent projects
        </button>
        <button type="button" style={navItem} onClick={() => navigate('projects')}>
          All projects
        </button>
        <button
          type="button"
          style={navItem}
          disabled={!projectId}
          title={projectId ? undefined : 'open a project first'}
          onClick={() => navigate('record')}
        >
          Recordings
        </button>
        <button type="button" style={navItem} onClick={() => navigate('library')}>
          Sound library
        </button>
        <button type="button" style={navItem} onClick={() => navigate('jobs')}>
          Background jobs
        </button>
        <div style={{ flex: 1 }} />
        <StatusBadge status="saved" label="No account needed" />
        <button type="button" style={{ ...navItem, marginTop: 8 }} onClick={() => navigate('setup')}>
          Audio settings
        </button>
      </nav>

      {/* Main column */}
      <div style={{ flex: 1, minWidth: 0, overflowY: 'auto', padding: tokens.space24 }}>
        <header style={{ marginBottom: tokens.space24 }}>
          <h1
            style={{
              margin: 0,
              fontSize: 34,
              fontWeight: 700,
              lineHeight: 1.2,
              color: tokens.text,
              fontFamily: tokens.fontDisplay,
            }}
          >
            Make room for
            <br />
            the next idea.
          </h1>
          <p style={{ margin: `${tokens.space8} 0 0`, color: tokens.muted, fontSize: 13 }}>
            Start with a recording, a sound, or a blank canvas.
          </p>
          <div style={{ display: 'flex', gap: tokens.space8, marginTop: tokens.space16, flexWrap: 'wrap' }}>
            <ActionButton variant="primary" onClick={() => navigate('new-project')}>
              + New project
            </ActionButton>
          </div>
          <div style={{ marginTop: tokens.space12, maxWidth: 560 }}>
            <Field label="Open project" hint="container directory on this device">
              {(id) => (
                <div style={{ display: 'flex', gap: tokens.space8 }}>
                  <TextInput
                    id={id}
                    value={openDir}
                    onChange={(ev) => setOpenDir(ev.target.value)}
                    placeholder="./projects/my-project.void"
                    disabled={!attached}
                  />
                  <ActionButton
                    variant="secondary"
                    loading={busy === 'path'}
                    disabled={!attached || openDir.trim() === '' || busy !== null}
                    title={attached ? undefined : 'engine not attached'}
                    onClick={() => void openByPath()}
                  >
                    Open
                  </ActionButton>
                </div>
              )}
            </Field>
          </div>
        </header>

        <section aria-label="Recent projects">
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: tokens.space12,
              marginBottom: tokens.space12,
            }}
          >
            <h2 style={{ margin: 0, fontSize: 20, fontWeight: 500, color: tokens.text, fontFamily: tokens.fontDisplay }}>
              Pick up where you left off.
            </h2>
            <div style={{ flex: 1 }} />
            <StatusBadge status={attached ? 'ready' : 'unavailable'} label={attached ? 'On this device' : 'No engine'} />
          </div>

          {entries.length === 0 ? (
            <div
              role="status"
              style={{
                padding: tokens.space24,
                borderRadius: tokens.radius8,
                border: `1px dashed ${tokens.line}`,
                color: tokens.muted,
                fontSize: 13,
                background: tokens.surface,
              }}
            >
              No recent projects on this device yet. Create a new project or open one by path — it shows up here after it
              opens.
            </div>
          ) : (
            <div role="list" style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
              {entries.map((e) => (
                <RecentProjectRow
                  key={`${e.containerDir}:${e.projectId}`}
                  name={e.name}
                  containerDir={e.containerDir}
                  kindLabel={e.kind === 'created' ? 'Created here' : 'Opened before'}
                  lastOpenedLabel={new Date(e.lastOpenedAt).toLocaleString()}
                  openLabel="Open"
                  removeLabel="Forget"
                  busy={busy === e.containerDir}
                  disabled={!attached || busy !== null}
                  onOpen={() => void openRecent(e.containerDir, e.projectId, e.name)}
                  onRemove={() => recents().getState().actions.remove(e.containerDir)}
                />
              ))}
            </div>
          )}
          <p style={{ marginTop: tokens.space12, fontSize: 11, color: tokens.subtle }}>
            Recent projects are local to this device. Cloud sync is not required or enabled.
            {persistError ? ` Recents could not be saved: ${persistError}` : ''}
          </p>
          {error ? (
            <p role="alert" style={{ marginTop: tokens.space8, fontSize: 12, color: tokens.danger }}>
              {error}
            </p>
          ) : null}
        </section>
      </div>
    </div>
  );
}
