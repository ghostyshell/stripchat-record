import { useCallback, useEffect, useMemo, useState } from "react";
import { api, listen } from "./api";
import type { Job, Snapshot } from "./types";
import { PAGE_TITLES } from "./types";
import { Bar, Chip, fmtBytes } from "./ui";
import { Circle, Film, FolderOpen, Library as LibraryIcon, Settings as SettingsIcon } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import logo from "./assets/logo.png";
import Record from "./pages/Record";
import Library from "./pages/Library";
import Convert from "./pages/Convert";
import Settings from "./pages/Settings";

type Page = "record" | "library" | "convert" | "settings";

const NAV: { id: Page; label: string; icon: LucideIcon }[] = [
  { id: "record", label: "Record", icon: Circle },
  { id: "library", label: "Library", icon: LibraryIcon },
  { id: "convert", label: "Convert", icon: Film },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

export interface PageProps {
  snap: Snapshot;
  reload: () => void;
  toast: (message: string) => void;
  refreshKey: number;
}

export default function App() {
  const [page, setPage] = useState<Page>("record");
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  const toast = useCallback((message: string) => {
    setToastMsg(message);
    window.setTimeout(() => setToastMsg((cur) => (cur === message ? null : cur)), 4200);
  }, []);

  const reload = useCallback(() => setRefreshKey((k) => k + 1), []);

  const loadSnapshot = useCallback(async () => {
    try {
      setSnap(await api.snapshot());
    } catch (err) {
      console.error("snapshot failed", err);
    }
  }, []);

  useEffect(() => {
    void loadSnapshot();
    const unlisteners: Promise<() => void>[] = [
      listen<{ id: number; done: number; total: number }>("job://progress", (update) => {
        setSnap((cur) =>
          cur
            ? {
                ...cur,
                jobs: cur.jobs.map((job) =>
                  job.id === update.id
                    ? { ...job, done_bytes: update.done, total_bytes: update.total }
                    : job
                ),
              }
            : cur
        );
      }),
      listen<Job>("job://done", (job) => {
        setSnap((cur) =>
          cur ? { ...cur, jobs: cur.jobs.map((j) => (j.id === job.id ? job : j)) } : cur
        );
        if (job.state === "done") toast(`${job.label}: ${job.message || "done"}`);
        else if (job.state === "failed") toast(`${job.label} failed: ${job.message}`);
        setRefreshKey((k) => k + 1);
      }),
    ];
    return () => {
      void Promise.all(unlisteners).then((uns) => uns.forEach((fn) => fn()));
    };
  }, [loadSnapshot, toast]);

  const running = useMemo(
    () => snap?.jobs.filter((j) => j.state === "running") ?? [],
    [snap]
  );

  if (!snap) {
    return (
      <div className="shell" aria-busy="true">
        <div className="backdrop">
          <div className="orb cyan" />
          <div className="orb indigo" />
          <div className="orb brass" />
        </div>
      </div>
    );
  }

  const pageProps: PageProps = { snap, reload, toast, refreshKey };

  return (
    <div className="shell">
      <div className="backdrop" aria-hidden="true">
        <div className="orb cyan" />
        <div className="orb indigo" />
        <div className="orb brass" />
      </div>

      <a className="skip-link" href="#workspace">
        Skip to content
      </a>

      <header className="topbar">
        <div className="brand">
          <img className="mark" src={logo} alt="" aria-hidden="true" />
          <div>
            <div className="eyebrow">STRIPCHAT RECORDER</div>
            <div className="page-title" aria-live="polite">
              {PAGE_TITLES[page]}
            </div>
          </div>
        </div>
        <div className="spacer" />
        {running.length > 0 ? (
          <Chip tone="good" live>
            Recording
          </Chip>
        ) : (
          <Chip tone="neutral">Idle</Chip>
        )}
      </header>

      <nav className="rail" aria-label="Sections">
        <div className="rail-caption">Room</div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className={`rail-item ${page === item.id ? "on" : ""}`}
            aria-current={page === item.id ? "page" : undefined}
            onClick={() => setPage(item.id)}
            onKeyDown={(e) => {
              if (e.altKey && e.key >= "1" && e.key <= "4") {
                const idx = Number(e.key) - 1;
                if (NAV[idx]) setPage(NAV[idx].id);
              }
            }}
          >
            <item.icon className="glyph-icon" size={17} aria-hidden="true" />
            <span className="label">{item.label}</span>
          </button>
        ))}
      </nav>

      <main className="workspace with-inspector" id="workspace" tabIndex={-1}>
        {page === "record" && <Record {...pageProps} />}
        {page === "library" && <Library {...pageProps} />}
        {page === "convert" && <Convert {...pageProps} />}
        {page === "settings" && <Settings {...pageProps} />}
      </main>

      <aside className="inspector" aria-label="Session">
        <div className="session-card">
          <h3>Output</h3>
          <div className="kv">
            <span className="k">folder</span>
            <span className="v" title={snap.output_dir}>
              {snap.output_dir.split("/").pop() || snap.output_dir}
            </span>
          </div>
          <div className="kv">
            <span className="k">split</span>
            <span className="v">
              {snap.config.split_size_bytes > 0
                ? fmtBytes(snap.config.split_size_bytes)
                : "off"}
            </span>
          </div>
          <div className="kv">
            <span className="k">keys</span>
            <span className="v">{snap.keys} pair(s)</span>
          </div>
          <div style={{ marginTop: 10 }}>
            <button className="btn small" onClick={() => void api.openOutputDir()}>
              <FolderOpen size={14} aria-hidden="true" /> Open folder
            </button>
          </div>
        </div>

        <div className="queue-card">
          <div className="panel-head" style={{ marginBottom: 0 }}>
            <h3 style={{ margin: 0 }}>Queue</h3>
            <div className="spacer" />
            {snap.jobs.some((j) => j.state !== "running") ? (
              <button
                className="btn small"
                onClick={async () => {
                  await api.jobsClear();
                  void loadSnapshot();
                }}
              >
                Clear
              </button>
            ) : null}
          </div>
          <div className="rows">
            {snap.jobs.length === 0 ? (
              <p style={{ margin: 0, fontSize: 12.5, color: "var(--text-faint)" }}>
                Nothing queued. Recordings and conversions show up here.
              </p>
            ) : (
              snap.jobs.slice(0, 8).map((job) => (
                <JobRow
                  key={job.id}
                  job={job}
                  onCancel={async () => {
                    await api.jobCancel(job.id);
                    void loadSnapshot();
                  }}
                />
              ))
            )}
          </div>
        </div>
      </aside>

      <footer className="statusbar">
        <span>
          <span className={`status-dot ${snap.ffmpeg ? "ok" : "idle"}`} />
          ffmpeg {snap.ffmpeg ? "ready" : "missing"}
        </span>
        <span>
          <span className={`status-dot ${snap.keys > 0 ? "ok" : "off"}`} />
          {snap.keys} key pair(s)
        </span>
        <span className="grow" />
        <span>stripchat-recorder v{snap.version}</span>
      </footer>

      {toastMsg ? (
        <div className="toast" role="status" aria-live="polite">
          <span className="chip laser">
            <span className="dot" />
            Recorder
          </span>
          {toastMsg}
        </div>
      ) : null}
    </div>
  );
}

function JobRow({ job, onCancel }: { job: Job; onCancel: () => void }) {
  const isRunning = job.state === "running";
  const liveRecording = isRunning && job.total_bytes === 0;
  return (
    <div className="job-row">
      <div className="head">
        <span className="label" title={job.label}>
          {job.label}
        </span>
        {isRunning ? (
          <button className="btn small" onClick={onCancel} title="Cancel job">
            Cancel
          </button>
        ) : (
          <Chip tone={job.state === "done" ? "good" : job.state === "failed" ? "bad" : "warn"}>
            {job.state}
          </Chip>
        )}
      </div>
      {isRunning ? (
        <div style={{ marginTop: 6 }}>
          {liveRecording ? (
            <div className="job-live">
              <span className="pulse" aria-hidden="true" />
              {fmtBytes(job.done_bytes)} recorded
            </div>
          ) : (
            <Bar value={job.done_bytes} max={job.total_bytes} live />
          )}
        </div>
      ) : (
        <div className="msg">{job.message}</div>
      )}
    </div>
  );
}
