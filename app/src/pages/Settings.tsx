import { useState } from "react";
import type { PageProps } from "../App";
import { api, pickers } from "../api";
import { Btn, Chip, Eyebrow, Toggle } from "../ui";
import { FolderOpen, Plus, RefreshCw, Trash2 } from "lucide-react";

function parseSize(raw: string): number {
  const value = raw.trim().toLowerCase();
  if (!value || /^(0|none|off)$/.test(value)) return 0;
  const match = value.match(/^([\d.]+)\s*([kmg]?)(i?b)?$/);
  if (!match) return 1024 * 1024 * 1024;
  const amount = Number(match[1]);
  const unit = match[2];
  const mult = unit === "g" ? 1024 ** 3 : unit === "m" ? 1024 ** 2 : unit === "k" ? 1024 : 1;
  return Math.max(0, Math.round(amount * mult));
}

function showSize(bytes: number): string {
  if (bytes <= 0) return "off";
  if (bytes % 1024 ** 3 === 0) return `${bytes / 1024 ** 3}G`;
  if (bytes % 1024 ** 2 === 0) return `${bytes / 1024 ** 2}M`;
  return `${bytes}`;
}

export default function SettingsPage({ snap, reload, toast }: PageProps) {
  const config = snap.config;
  const [split, setSplit] = useState(showSize(config.split_size_bytes));
  const [proxy, setProxy] = useState(config.proxy);
  const [pkey, setPkey] = useState("");
  const [pdkey, setPdkey] = useState("");

  async function save(patch: Partial<typeof config>) {
    try {
      await api.configSet(patch);
      reload();
    } catch (err) {
      toast(String(err));
    }
  }

  return (
    <div className="page">
      <header className="page-head">
        <Eyebrow>Preferences</Eyebrow>
        <h1>Settings</h1>
        <p className="lede">Where files go, how they are cut and compressed, and the keys that unlock the CDN.</p>
      </header>

      <section className="panel">
        <h3>Output</h3>
        <div className="field">
          <label>Recordings folder</label>
          <div className="row">
            <input className="input grow" value={config.output_dir} readOnly placeholder={snap.output_dir} aria-label="Recordings folder" />
            <Btn
              onClick={async () => {
                const dir = await pickers.pickDirectory("Choose the recordings folder");
                if (dir) await save({ output_dir: dir });
              }}
            >
              <FolderOpen size={14} aria-hidden="true" /> Choose
            </Btn>
            <Btn onClick={() => void api.openOutputDir()}>Open</Btn>
          </div>
        </div>
        <div className="grid-2">
          <div className="field">
            <label htmlFor="split">Split parts at</label>
            <div className="row">
              <input
                id="split"
                className="input"
                value={split}
                onChange={(e) => setSplit(e.target.value)}
                onBlur={() => void save({ split_size_bytes: parseSize(split) })}
              />
              <span className="unit">e.g. 1G, 512M, off</span>
            </div>
          </div>
          <div className="field">
            <label htmlFor="retries">Segment retries</label>
            <input
              id="retries"
              className="input"
              type="number"
              min={0}
              max={10}
              value={config.segment_retries}
              onChange={(e) => void save({ segment_retries: Number(e.target.value) })}
            />
          </div>
        </div>
        <Toggle
          checked={config.keep_ts}
          onChange={(next) => void save({ keep_ts: next, remux_mp4: true })}
          label="keep the .recording.mp4 next to the final file"
        />
      </section>

      <section className="panel">
        <h3>Conversion</h3>
        <Toggle
          checked={config.convert_on_finish}
          onChange={(next) => void save({ convert_on_finish: next })}
          label="compress every recording when it stops"
        />
        <div className="field">
          <label htmlFor="cpreset">Default preset</label>
          <select
            id="cpreset"
            className="input"
            value={config.convert_preset}
            onChange={(e) => void save({ convert_preset: e.target.value })}
          >
            <option value="copy">Keep original (remux only)</option>
            <option value="h264_balanced">H.264 balanced</option>
            <option value="h265_balanced">H.265 balanced</option>
            <option value="h265_compact">H.265 compact</option>
            <option value="h264_720">H.264 720p</option>
          </select>
        </div>
      </section>

      <section className="panel">
        <h3>Network</h3>
        <div className="field">
          <label htmlFor="proxy">Proxy for the site API</label>
          <div className="row">
            <input
              id="proxy"
              className="input grow"
              value={proxy}
              placeholder="socks5h://127.0.0.1:9050"
              onChange={(e) => setProxy(e.target.value)}
              onBlur={() => void save({ proxy: proxy.trim() })}
            />
            <Btn onClick={() => void save({ proxy: proxy.trim() })}>Save</Btn>
          </div>
          <p className="hint">
            Cloudflare refuses the room lookup from some addresses. A SOCKS5 proxy (a
            local Tor, for example) fixes it. Media segments always go direct.
          </p>
        </div>
      </section>

      <section className="panel">
        <h3>Encryption keys</h3>
        <p className="hint">
          Stripchat hides segment paths behind a rotating pkey/pdkey pair. Known pairs
          ship with the app; add a fresh pair here when playback stops working.
        </p>
        <div className="row">
          <input
            className="input"
            placeholder="pkey"
            aria-label="pkey"
            value={pkey}
            onChange={(e) => setPkey(e.target.value.trim())}
          />
          <input
            className="input"
            placeholder="pdkey"
            aria-label="pdkey"
            value={pdkey}
            onChange={(e) => setPdkey(e.target.value.trim())}
          />
          <Btn
            disabled={!pkey || !pdkey}
            onClick={async () => {
              await save({ pdkeys: { ...config.pdkeys, [pkey]: pdkey } });
              setPkey("");
              setPdkey("");
            }}
          >
            <Plus size={14} aria-hidden="true" /> Add
          </Btn>
        </div>
        {Object.keys(config.pdkeys).length > 0 ? (
          <div className="table">
            {Object.entries(config.pdkeys).map(([k, v]) => (
              <div className="table-row" key={k}>
                <span className="mono">{k}</span>
                <span className="mono">{v.slice(0, 4)}...</span>
                <span />
                <span className="actions">
                  <Btn
                    small
                    variant="danger"
                    onClick={() => {
                      const next = { ...config.pdkeys };
                      delete next[k];
                      void save({ pdkeys: next });
                    }}
                  >
                    <Trash2 size={13} aria-hidden="true" />
                  </Btn>
                </span>
              </div>
            ))}
          </div>
        ) : null}
      </section>

      <section className="panel">
        <h3>Tools</h3>
        <div className="grid-2">
          <div className="field">
            <label>ffmpeg path</label>
            <input
              className="input"
              value={config.ffmpeg_path}
              aria-label="ffmpeg path"
              placeholder="found on PATH"
              onChange={(e) => void save({ ffmpeg_path: e.target.value })}
            />
          </div>
          <div className="field">
            <label>ffprobe path</label>
            <input
              className="input"
              value={config.ffprobe_path}
              aria-label="ffprobe path"
              placeholder="found on PATH"
              onChange={(e) => void save({ ffprobe_path: e.target.value })}
            />
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="row between">
          <h3 style={{ margin: 0 }}>Health</h3>
          <Btn small onClick={reload}>
            <RefreshCw size={13} aria-hidden="true" /> Re-run
          </Btn>
        </div>
        <div className="checks">
          {snap.doctor.map((check) => (
            <div className={`check ${check.ok ? "ok" : "bad"}`} key={check.name}>
              <Chip tone={check.ok ? "good" : "bad"}>{check.ok ? "ok" : "fix"}</Chip>
              <div>
                <strong>{check.name}</strong>
                <span className="mono">{check.detail}</span>
                {!check.ok && check.fix ? <p className="hint">{check.fix}</p> : null}
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
