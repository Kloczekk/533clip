export type ClipStatus = "processing" | "ready" | "failed";

export interface ClipDetectedPayload {
  id: string;
  filePath: string;
  fileName: string;
  createdAt: string;
  gameName?: string;
}

export interface Clip {
  id: string;
  filePath: string;
  fileName: string;
  displayName?: string;
  gameName?: string;
  createdAt: string;
  duration?: number;
  audioPeaks?: number[];
  waveform?: number[];
  resolution?: string;
  thumbnailPath?: string;
  isFavorite: boolean;
  tags: string[];
  status: ClipStatus;
  shareUrl?: string;
  shareKey?: string;
  sharedAt?: string;
}

