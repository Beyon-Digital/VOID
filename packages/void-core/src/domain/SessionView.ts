export interface SessionClip {
  id: string;
  name: string;
  /** Explicit slot membership: sceneId and trackId are stored relationships.
   *  Scene membership is NEVER derived from clip/track/scene ID prefixes. */
  sceneId: string;
  trackId: string;
  startBeat: number;
  length: number;
  loop: boolean;
  color?: string;
  type: 'audio' | 'midi';
}

export interface Scene {
  id: string;
  name: string;
  tempo?: number;
  timeSignature?: string;
}

export interface ClipMatrix {
  clips: SessionClip[];
  scenes: Scene[];
}

export interface SessionView {
  matrix: ClipMatrix;
}

/**
 * Group clips by their explicit sceneId relationship.
 * Clips whose sceneId does not match a present scene land in no group;
 * they are never assigned by ID-prefix matching.
 */
export function groupClipsByScene(matrix: ClipMatrix): Map<string, SessionClip[]> {
  const byScene = new Map<string, SessionClip[]>();
  for (const scene of matrix.scenes) {
    byScene.set(scene.id, []);
  }
  for (const clip of matrix.clips) {
    byScene.get(clip.sceneId)?.push(clip);
  }
  return byScene;
}
