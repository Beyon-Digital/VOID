// Adapter: implements AudioPort
//
// WARNING — NOT IMPLEMENTED AUDIO. This is a browser-AudioContext stub from
// the pre-engine prototype. It does not connect to the VOID engine worker:
// transport methods issue no engine commands, analysers/peaks always return
// zeros, and track/clip calls mutate nothing. It must never be counted as
// real audio — real audio lives in the C++/Tracktion worker and real
// transport/meters flow over void://control and void://telemetry. Kept only
// to satisfy the AudioPort interface while prototype code is ported.
import { AudioPort, AudioTrackRef, ClipRef, AudioNodeRef, TrackType, ClipData } from 'void-core';

export class WebAudioAdapter implements AudioPort {
  private context: AudioContext;
  private analyserDataCache: Map<string, Float32Array> = new Map();

  constructor() {
    this.context = new window.AudioContext();
  }

  play(): void {
    if (this.context.state === 'suspended') {
      this.context.resume();
    }
    // Implement tone.js transport play integration
  }

  stop(): void {
    // Implement transport stop
  }

  pause(): void {
    this.context.suspend();
  }

  record(): void {}

  setTransportBPM(bpm: number): void {}
  setTimeSignature(num: number, denom: number): void {}
  setLoopRegion(startBeat: number, endBeat: number): void {}

  getCurrentBeat(): number { return 0; }
  getCurrentTime(): number { return this.context.currentTime; }

  createTrack(type: TrackType, id: string): AudioTrackRef {
    return id;
  }

  deleteTrack(id: string): void {
    // ⚡ Bolt: Cleanup cache to prevent memory leak
    this.analyserDataCache.delete(id);
  }

  setTrackVolume(id: string, volume: number): void {}
  setTrackPan(id: string, pan: number): void {}
  setTrackMute(id: string, muted: boolean): void {}
  setTrackSolo(id: string, soloed: boolean): void {}
  routeTrackTo(srcId: string, dstId: string): void {}

  placeClip(trackId: string, clip: ClipData, startBeat: number): ClipRef {
    return clip.id;
  }

  removeClip(clipId: string): void {}
  trimClip(clipId: string, startOffset: number, endOffset: number): void {}

  connectNode(src: AudioNodeRef, dst: AudioNodeRef, srcOutput?: number, dstInput?: number): void {}
  disconnectNode(src: AudioNodeRef, dst: AudioNodeRef): void {}

  createAudioNode(type: string, params: Record<string, number>): AudioNodeRef {
    return `${type}-node`;
  }

  getAnalyserData(trackId: string, fftSize: number): Float32Array {
    // ⚡ Bolt: Cache TypedArray (Float32Array) instead of instantiating a new one on every call
    // This prevents severe garbage collection pressure and UI micro-stutters during high-frequency polling
    // NOTE: zero-filled — never wire this to visible meters as live data.
    let cached = this.analyserDataCache.get(trackId);
    if (!cached || cached.length !== fftSize) {
      cached = new Float32Array(fftSize);
      this.analyserDataCache.set(trackId, cached);
    }
    return cached;
  }

  getPeakLevel(trackId: string): { peak: number; rms: number } {
    // Always zero — stub only; real levels come from MeterFrame telemetry.
    return { peak: 0, rms: 0 };
  }
}
