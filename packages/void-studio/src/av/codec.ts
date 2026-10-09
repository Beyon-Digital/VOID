// Declarative codec matrix — TS twin of crates/void-av::codec
// (W24, T88). The UI gates the same table: a codec with no matching
// ffmpeg build or with rights outside the session's allowed set is NOT
// presented as selectable. There is no pretend-supported flag — an
// unavailable codec produces a typed codec_unavailable outcome.

/** Whether the codec's output may be used outside dev pipelines. */
export type AvCodecRights = 'cleared' | 'development_only';

export interface AvCodecSpec {
  id: string;
  containerExt: string;
  videoEncoder: string;
  audioEncoder: string;
  pixelFormat: string;
  rights: AvCodecRights;
  licenseNote: string;
}

/**
 * Mirrors CODEC_MATRIX in crates/void-av — keep rows in lockstep.
 * `development_only` means the codec commonly ships in ffmpeg builds
 * but carries patent/licensing obligations for redistribution; the
 * studio may still select it for local work, and provenance records
 * the flag so downstream consumers can gate it.
 */
export const AV_CODEC_MATRIX: readonly AvCodecSpec[] = [
  {
    id: 'ffv1_flac_mkv',
    containerExt: 'mkv',
    videoEncoder: 'ffv1',
    audioEncoder: 'flac',
    pixelFormat: 'yuv420p',
    rights: 'cleared',
    licenseNote: 'FFV1 + FLAC in Matroska — fully open, distribution-cleared.',
  },
  {
    id: 'vp9_opus_webm',
    containerExt: 'webm',
    videoEncoder: 'libvpx-vp9',
    audioEncoder: 'libopus',
    pixelFormat: 'yuv420p',
    rights: 'cleared',
    licenseNote: 'VP9 + Opus in WebM — royalty-free, distribution-cleared.',
  },
  {
    id: 'h264_aac_mp4',
    containerExt: 'mp4',
    videoEncoder: 'libx264',
    audioEncoder: 'aac',
    pixelFormat: 'yuv420p',
    rights: 'development_only',
    licenseNote: 'H.264 + AAC — MPEG-LA licensing applies to distributed output.',
  },
  {
    id: 'prores_pcm_mov',
    containerExt: 'mov',
    videoEncoder: 'prores_ks',
    audioEncoder: 'pcm_s24le',
    pixelFormat: 'yuv422p10le',
    rights: 'development_only',
    licenseNote: 'ProRes + PCM — Apple patent licensing applies to distributed output.',
  },
] as const;

export function avCodec(id: string): AvCodecSpec | undefined {
  return AV_CODEC_MATRIX.find((c) => c.id === id);
}

/**
 * Gate report — mirrors `CodecGateReport`. `selectable=false` is the
 * ONLY "not supported" state the UI ever presents.
 */
export interface AvCodecGateReport {
  codecId: string;
  ffmpegPresent: boolean;
  videoEncoderPresent: boolean;
  audioEncoderPresent: boolean;
  selectable: boolean;
  reason: string | null;
}

export function avCodecGate(
  codecId: string,
  ffmpegPresent: boolean,
  encodersPresent: (name: string) => boolean,
): AvCodecGateReport {
  const row = avCodec(codecId);
  if (!row) {
    return {
      codecId, ffmpegPresent, videoEncoderPresent: false,
      audioEncoderPresent: false, selectable: false,
      reason: `unknown codec '${codecId}'`,
    };
  }
  if (!ffmpegPresent) {
    return {
      codecId, ffmpegPresent, videoEncoderPresent: false,
      audioEncoderPresent: false, selectable: false,
      reason: 'ffmpeg binary not present',
    };
  }
  const v = encodersPresent(row.videoEncoder);
  const a = encodersPresent(row.audioEncoder);
  if (!v || !a) {
    return {
      codecId, ffmpegPresent, videoEncoderPresent: v,
      audioEncoderPresent: a, selectable: false,
      reason: `ffmpeg build lacks ${!v ? row.videoEncoder : row.audioEncoder}`,
    };
  }
  return {
    codecId, ffmpegPresent, videoEncoderPresent: true,
    audioEncoderPresent: true, selectable: true, reason: null,
  };
}

/** All codecs the current ffmpeg build can actually encode — the
 * only list a picker may offer. */
export function selectableAvCodecs(
  ffmpegPresent: boolean,
  encodersPresent: (name: string) => boolean,
): AvCodecSpec[] {
  return AV_CODEC_MATRIX.filter(
    (c) => avCodecGate(c.id, ffmpegPresent, encodersPresent).selectable,
  );
}
