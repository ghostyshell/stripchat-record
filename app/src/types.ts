export interface Config {
  output_dir: string;
  split_size_bytes: number;
  remux_mp4: boolean;
  convert_on_finish: boolean;
  convert_preset: string;
  keep_ts: boolean;
  segment_retries: number;
  poll_interval_secs: number;
  ffmpeg_path: string;
  ffprobe_path: string;
  models: string[];
  pdkeys: Record<string, string>;
  proxy: string;
}

export type JobState = "running" | "done" | "failed" | "cancelled";

export interface Job {
  id: number;
  kind: string;
  label: string;
  state: JobState;
  done_bytes: number;
  total_bytes: number;
  message: string;
  started_at: number;
  finished_at: number | null;
}

export interface Check {
  name: string;
  ok: boolean;
  detail: string;
  fix: string;
}

export interface Snapshot {
  config: Config;
  jobs: Job[];
  doctor: Check[];
  keys: number;
  ffmpeg: boolean;
  output_dir: string;
  version: string;
}

export interface Variant {
  name: string;
  resolution: string;
  bandwidth: number;
}

export interface CamView {
  model: string;
  live: boolean;
  stream_name: string | null;
  variants: Variant[];
  message: string;
}

export interface Recording {
  name: string;
  path: string;
  size_bytes: number;
  modified: number;
  in_progress: boolean;
}

export interface Preset {
  id: string;
  label: string;
  detail: string;
}

export const PRESETS: Preset[] = [
  { id: "copy", label: "Keep original", detail: "Remux only. No quality loss." },
  { id: "h264_balanced", label: "H.264 balanced", detail: "CRF 23, plays everywhere." },
  { id: "h265_balanced", label: "H.265 balanced", detail: "CRF 26, smaller, slower." },
  { id: "h265_compact", label: "H.265 compact", detail: "CRF 30, smallest files." },
  { id: "h264_720", label: "H.264 720p", detail: "Scale to 720p, CRF 24." },
];

export const PAGE_TITLES: Record<string, string> = {
  record: "Record",
  library: "Library",
  convert: "Convert",
  settings: "Settings",
};
