// Typed bridge to the Tauri runtime. In ?mock=1 mode mock.ts installs
// window.__TAURI_INTERNALS__ so this file works unchanged.

import type { CamView, Check, Config, Job, Recording, Snapshot } from "./types";

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const internals = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
  return internals.invoke(cmd, args);
}

type UnlistenFn = () => void;

export function listen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  const api = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
  const wrapped = (e: { payload: T }) => handler(e.payload);
  return api
    .invoke("plugin:event|listen", {
      event,
      handler: api.transformCallback(wrapped),
      target: { kind: "Any" },
    })
    .then((id: number) => {
      return () => {
        void api.invoke("plugin:event|unlisten", { event, eventId: id });
      };
    });
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  camInfo: (model: string) => invoke<CamView>("cam_info", { model }),

  recordStart: (
    model: string,
    quality: string,
    durationSecs: number | null,
    convert: string | null,
    keepSource: boolean
  ) =>
    invoke<number>("record_start", {
      model,
      quality,
      durationSecs,
      convert,
      keepSource,
    }),
  convertStart: (file: string, preset: string, output: string | null) =>
    invoke<number>("convert_start", { file, preset, output }),

  libraryList: () => invoke<Recording[]>("library_list"),
  libraryDelete: (path: string) => invoke<void>("library_delete", { path }),
  libraryReveal: (path: string) => invoke<void>("library_reveal", { path }),
  libraryFinalize: (path: string) => invoke<number>("library_finalize", { path }),

  configGet: () => invoke<Config>("config_get"),
  configSet: (patch: Partial<Config>) => invoke<Config>("config_set", { patch }),

  doctorRun: () => invoke<Check[]>("doctor_run"),
  jobsList: () => invoke<Job[]>("jobs_list"),
  jobCancel: (id: number) => invoke<boolean>("job_cancel", { id }),
  jobsClear: () => invoke<void>("jobs_clear"),
  openOutputDir: () => invoke<string>("open_output_dir"),
};

export const pickers = {
  async pickFiles(title: string, extensions: string[]): Promise<string[]> {
    const api = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
    const result = (await api.invoke("plugin:dialog|open", {
      options: { title, multiple: true, filters: [{ name: "Files", extensions }] },
    })) as string | string[] | null;
    if (!result) return [];
    return Array.isArray(result) ? result : [result];
  },
  async pickFile(title: string, extensions: string[]): Promise<string | null> {
    const files = await pickers.pickFiles(title, extensions);
    return files.length ? files[0] : null;
  },
  async pickDirectory(title: string): Promise<string | null> {
    const api = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
    const result = (await api.invoke("plugin:dialog|open", {
      options: { title, directory: true, multiple: false },
    })) as string | null;
    return result ?? null;
  },
  async pickSavePath(title: string, defaultName: string, extensions: string[]): Promise<string | null> {
    const api = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
    const result = (await api.invoke("plugin:dialog|save", {
      options: { title, defaultPath: defaultName, filters: [{ name: "Video", extensions }] },
    })) as string | null;
    return result ?? null;
  },
};
