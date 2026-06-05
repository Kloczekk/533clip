export type ClipStatus = "processing" | "ready" | "failed";

export interface ClipDetectedPayload {
  id: string;
  filePath: string;
  fileName: string;
  createdAt: string;
}

export interface Clip {
  id: string;
  filePath: string;
  fileName: string;
  displayName?: string;
  createdAt: string;
  duration?: number;
  resolution?: string;
  thumbnailPath?: string;
  isFavorite: boolean;
  tags: string[];
  status: ClipStatus;
}

