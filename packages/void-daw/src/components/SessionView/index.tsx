import React from 'react';
import { ClipMatrix, SessionClip, Scene, groupClipsByScene } from 'void-core';

interface SessionViewProps {
  matrix: ClipMatrix;
  onClipTrigger?: (clipId: string) => void;
  onSceneTrigger?: (sceneId: string) => void;
}

const EMPTY_CLIPS: SessionClip[] = [];

interface SceneColumnProps {
  scene: Scene;
  sceneClips: SessionClip[];
  onSceneTrigger?: (sceneId: string) => void;
  onClipTrigger?: (clipId: string) => void;
}

const SceneColumn: React.FC<SceneColumnProps> = React.memo(({ scene, sceneClips, onSceneTrigger, onClipTrigger }) => (
  <div key={scene.id} style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
    <button
      onClick={() => onSceneTrigger?.(scene.id)}
      style={{ padding: '8px', backgroundColor: 'var(--void-surface)', color: 'white', border: '1px solid var(--void-border)' }}
    >
      {scene.name}
    </button>
    {sceneClips.map(clip => (
       <button
        key={clip.id}
        onClick={() => onClipTrigger?.(clip.id)}
        style={{ padding: '24px', backgroundColor: clip.color || 'var(--void-accent)', color: 'white', border: 'none', borderRadius: '4px' }}
       >
         {clip.name}
       </button>
    ))}
  </div>
));
SceneColumn.displayName = 'SceneColumn';

// ⚡ Bolt: Wrapped in React.memo to prevent unnecessary re-renders
export const SessionView: React.FC<SessionViewProps> = React.memo(({ matrix, onClipTrigger, onSceneTrigger }) => {
  // Clips are placed in scenes by their explicit sceneId relationship only.
  // ID prefixes never determine membership (a clip named "a-clip" does not
  // belong to scene "a"). See groupClipsByScene in void-core.
  const clipsByScene = React.useMemo(() => groupClipsByScene(matrix), [matrix]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', padding: '16px' }}>
      <h2>Session View</h2>
      <div style={{ display: 'flex', gap: '16px' }}>
        {matrix.scenes.map(scene => {
          const sceneClips = clipsByScene.get(scene.id) || EMPTY_CLIPS;
          return (
            <SceneColumn
              key={scene.id}
              scene={scene}
              sceneClips={sceneClips}
              onSceneTrigger={onSceneTrigger}
              onClipTrigger={onClipTrigger}
            />
          );
        })}
      </div>
    </div>
  );
});

SessionView.displayName = 'SessionView';
