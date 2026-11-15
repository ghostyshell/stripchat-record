// Mock bridge: installs window.__TAURI_INTERNALS__ with fixture data so the
// UI runs in a plain browser (?mock=1) with no backend and no network.

import type { Job } from "./types";

type Handler = (payload: unknown) => void;

const listeners = new Map<number, { event: string; handler: Handler }>();
let nextCallbackId = 1;

function emit(event: string, payload: unknown) {
  for (const { event: name, handler } of listeners.values()) {
    if (name === event) handler({ event, id: nextCallbackId, payload });
  }
}

const GB = 1024 * 1024 * 1024;
const MB = 1024 * 1024;

let config = {
  output_dir: "/Users/demo/Movies/Stripchat Recorder",
  split_size_bytes: 1024 * 1024 * 1024,
  remux_mp4: true,
  convert_on_finish: false,
  convert_preset: "h264_balanced",
  keep_ts: false,
  segment_retries: 3,
  poll_interval_secs: 2,
  ffmpeg_path: "",
  ffprobe_path: "",
  models: ["roxyheartley", "Blushinblondie"],
  pdkeys: {},
  proxy: "",
};

const now = () => Math.floor(Date.now() / 1000);

let recordings = [
  {
    name: "roxyheartley-20261115-091204.mp4",
    path: "/Users/demo/Movies/Stripchat Recorder/roxyheartley-20261115-091204.mp4",
    size_bytes: 1.4 * GB,
    modified: now() - 3600,
    in_progress: false,
  },
  {
    name: "Blushinblondie-20261115-085412.mp4",
    path: "/Users/demo/Movies/Stripchat Recorder/Blushinblondie-20261115-085412.mp4",
    size_bytes: 640 * MB,
    modified: now() - 7200,
    in_progress: false,
  },
  {
    name: "roxyheartley-20261115-090001.recording.mp4",
    path: "/Users/demo/Movies/Stripchat Recorder/roxyheartley-20261115-090001.recording.mp4",
    size_bytes: 210 * MB,
    modified: now() - 5400,
    in_progress: true,
  },
];

let jobs: Job[] = [
  {
    id: 1,
    kind: "record",
    label: "Blushinblondie",
    state: "done" as const,
    done_bytes: 640 * MB,
    total_bytes: 0,
    message: "640.0 MB in 22m 14s (612 segment(s), stream ended)",
    started_at: now() - 7200,
    finished_at: now() - 5866,
  },
];

const variants = [
  { name: "source", resolution: "1280x720", bandwidth: 2990000 },
  { name: "240p", resolution: "426x240", bandwidth: 690000 },
];

const doctor = [
  { name: "ffmpeg", ok: true, detail: "/opt/homebrew/bin/ffmpeg", fix: "" },
  { name: "ffprobe", ok: true, detail: "/opt/homebrew/bin/ffprobe", fix: "" },
  {
    name: "output folder",
    ok: true,
    detail: "/Users/demo/Movies/Stripchat Recorder",
    fix: "",
  },
  { name: "encryption keys", ok: true, detail: "2 pkey/pdkey pair(s) loaded", fix: "" },
  { name: "Stripchat API", ok: true, detail: "direct", fix: "" },
];

function snapshot() {
  return {
    config,
    jobs,
    doctor,
    keys: 2,
    ffmpeg: true,
    output_dir: config.output_dir,
    version: "0.5.0",
  };
}

let nextJobId = 10;

function startRecording(model: string): number {
  const id = nextJobId++;
  const job: Job = {
    id,
    kind: "record",
    label: model,
    state: "running" as const,
    done_bytes: 0,
    total_bytes: 0,
    message: "connecting",
    started_at: now(),
    finished_at: null as number | null,
  };
  jobs = [job, ...jobs];
  let done = 0;
  const tick = window.setInterval(() => {
    done += 900000;
    job.done_bytes = done;
    emit("job://progress", { id, done, total: 0 });
    if (done > 12 * 1024 * 1024) {
      window.clearInterval(tick);
      job.state = "done";
      job.finished_at = now();
      job.message = "saved a demo recording.mp4";
      recordings = [
        {
          name: `${model}-mock.mp4`,
          path: `/Users/demo/Movies/Stripchat Recorder/${model}-mock.mp4`,
          size_bytes: done,
          modified: now(),
          in_progress: false,
        },
        ...recordings,
      ];
      emit("job://done", job);
    }
  }, 700);
  return id;
}

async function invoke(cmd: string, args: Record<string, any> = {}): Promise<unknown> {
  switch (cmd) {
    case "snapshot":
      return snapshot();
    case "cam_info":
      return {
        model: args.model,
        live: true,
        stream_name: "288906106",
        variants,
        message: "Live at 2 quality variant(s)",
      };
    case "record_start":
      return startRecording(String(args.model));
    case "convert_start": {
      const id = nextJobId++;
      const job: Job = {
        id,
        kind: "convert",
        label: String(args.file).split("/").pop() ?? "file",
        state: "running" as const,
        done_bytes: 0,
        total_bytes: 1000,
        message: "encoding",
        started_at: now(),
        finished_at: null as number | null,
      };
      jobs = [job, ...jobs];
      let done = 0;
      const tick = window.setInterval(() => {
        done += 125;
        job.done_bytes = done;
        emit("job://progress", { id, done, total: 1000 });
        if (done >= 1000) {
          window.clearInterval(tick);
          job.state = "done";
          job.finished_at = now();
          job.message = "saved converted.mp4";
          emit("job://done", job);
        }
      }, 400);
      return id;
    }
    case "library_list":
      return [...recordings].sort((a, b) => b.modified - a.modified);
    case "library_delete":
      recordings = recordings.filter((r) => r.path !== args.path);
      return null;
    case "library_reveal":
    case "open_output_dir":
      return null;
    case "library_finalize": {
      recordings = recordings.map((r) =>
        r.path === args.path
          ? { ...r, in_progress: false, name: r.name.replace(".recording", "") }
          : r
      );
      return 1;
    }
    case "config_get":
      return config;
    case "config_set":
      config = { ...config, ...args.patch };
      return config;
    case "doctor_run":
      return doctor;
    case "jobs_list":
      return jobs;
    case "job_cancel": {
      jobs = jobs.map((j) => (j.id === args.id ? { ...j, state: "cancelled" as const } : j));
      return true;
    }
    case "jobs_clear":
      jobs = jobs.filter((j) => j.state === "running");
      return null;
    case "plugin:dialog|open": {
      const options = (args.options ?? {}) as { directory?: boolean };
      if (options.directory) return "/Users/demo/Movies/Stripchat Recorder";
      return ["/Users/demo/Movies/Stripchat Recorder/roxyheartley-20261115-091204.mp4"];
    }
    case "plugin:dialog|save":
      return "/Users/demo/Movies/Stripchat Recorder/converted.mp4";
    case "plugin:event|listen": {
      // transformCallback already registered the handler under this id; only
      // the event name is new.
      const id = Number(args.handler);
      const entry = listeners.get(id);
      if (entry) entry.event = String(args.event);
      return id;
    }
    case "plugin:event|unlisten":
      listeners.delete(args.eventId);
      return null;
    default:
      console.warn("[mock] unhandled invoke:", cmd, args);
      return null;
  }
}

(window as unknown as Record<string, any>).__TAURI_INTERNALS__ = {
  invoke,
  transformCallback(cb: Handler) {
    const id = nextCallbackId++;
    listeners.set(id, { event: "__callback__", handler: cb });
    return id;
  },
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main" } },
  plugins: {},
};

console.info("[mock] Stripchat Recorder fixture bridge installed");

export {};
