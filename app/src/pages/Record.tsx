import { useMemo, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { CamView } from "../types";
import { PRESETS } from "../types";
import { Btn, Chip, Eyebrow, Empty, Toggle } from "../ui";
import { Radio, Search, Square } from "lucide-react";

const MODES = ["source", "auto"] as const;

export default function RecordPage({ snap, reload, toast, refreshKey }: PageProps) {
  const [model, setModel] = useState(snap.config.models[0] ?? "");
  const [cam, setCam] = useState<CamView | null>(null);
  const [checking, setChecking] = useState(false);
  const [quality, setQuality] = useState("source");
  const [duration, setDuration] = useState("");
  const [convert, setConvert] = useState(snap.config.convert_on_finish);
  const [preset, setPreset] = useState(snap.config.convert_preset);
  const [keepSource, setKeepSource] = useState(false);

  const tracked = useMemo(() => snap.config.models.slice(0, 12), [snap.config.models, refreshKey]);
  const running = snap.jobs.find((j) => j.state === "running" && j.kind === "record");

  async function check() {
    const name = model.trim();
    if (!name) return;
    setChecking(true);
    try {
      const info = await api.camInfo(name);
      setCam(info);
      if (info.live && info.variants.length) setQuality(info.variants[0].name);
    } catch (err) {
      toast(String(err));
    } finally {
      setChecking(false);
    }
  }

  async function start() {
    const name = model.trim();
    if (!name) {
      toast("Enter a model username first");
      return;
    }
    try {
      await api.recordStart(
        name,
        quality,
        duration.trim() ? Number(duration) : null,
        convert ? preset : null,
        keepSource
      );
      const models = [name, ...snap.config.models.filter((m) => m !== name)].slice(0, 12);
      await api.configSet({ models });
      reload();
      toast(`Recording ${name}`);
    } catch (err) {
      toast(String(err));
    }
  }

  const live = cam?.live ?? false;

  return (
    <div className="page">
      <header className="page-head">
        <Eyebrow>Direct from the CDN</Eyebrow>
        <h1>Record a live room</h1>
        <p className="lede">
          The CDN playlist is read directly and every segment is written to disk as it
          airs. No screen capture, no browser, no re-encode: the file is the source
          quality the room is broadcasting.
        </p>
      </header>

      <section className="panel">
        <div className="field">
          <label htmlFor="model">Model username</label>
          <div className="row">
            <input
              id="model"
              className="input grow"
              value={model}
              placeholder="the name in the stripchat.com URL"
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => {
                setModel(e.target.value);
                setCam(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") void check();
              }}
            />
            <Btn onClick={() => void check()} disabled={checking || !model.trim()}>
              <Search size={15} aria-hidden="true" /> {checking ? "Checking" : "Check"}
            </Btn>
          </div>
          {tracked.length > 0 ? (
            <div className="chips">
              {tracked.map((name) => (
                <button
                  key={name}
                  className={`tag ${name === model ? "on" : ""}`}
                  onClick={() => {
                    setModel(name);
                    setCam(null);
                  }}
                >
                  {name}
                </button>
              ))}
            </div>
          ) : null}
        </div>

        {cam ? (
          <div className={`cam-card ${live ? "live" : ""}`}>
            <div className="cam-head">
              <Chip tone={live ? "good" : "bad"} live={live}>
                {live ? "Live" : "Offline"}
              </Chip>
              <strong>{cam.model}</strong>
              <span className="cam-msg">{cam.message}</span>
            </div>
            {live && cam.variants.length > 0 ? (
              <div className="field">
                <label>Quality</label>
                <div className="row wrap">
                  {cam.variants.map((v) => (
                    <button
                      key={v.name}
                      className={`tag ${quality === v.name ? "on" : ""}`}
                      onClick={() => setQuality(v.name)}
                    >
                      {v.name} <span className="tag-sub">{v.resolution}</span>
                    </button>
                  ))}
                  {MODES.map((m) => (
                    <button
                      key={m}
                      className={`tag ${quality === m ? "on" : ""}`}
                      onClick={() => setQuality(m)}
                    >
                      {m}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        ) : null}

        <div className="grid-2">
          <div className="field">
            <label htmlFor="duration">Stop after</label>
            <div className="row">
              <input
                id="duration"
                className="input"
                type="number"
                min={5}
                placeholder="until the stream ends"
                value={duration}
                onChange={(e) => setDuration(e.target.value)}
              />
              <span className="unit">seconds</span>
            </div>
          </div>
          <div className="field">
            <label htmlFor="preset">After recording</label>
            <div className="row">
              <Toggle
                checked={convert}
                onChange={setConvert}
                label={convert ? "compress" : "remux only"}
              />
              {convert ? (
                <select
                  id="preset"
                  className="input"
                  value={preset}
                  onChange={(e) => setPreset(e.target.value)}
                >
                  {PRESETS.filter((p) => p.id !== "copy").map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.label}
                    </option>
                  ))}
                </select>
              ) : null}
            </div>
          </div>
        </div>

        <div className="row between end">
          <Toggle
            checked={keepSource}
            onChange={setKeepSource}
            label="keep the .recording.mp4 too"
          />
          <Btn variant="primary" onClick={() => void start()} disabled={!model.trim()}>
            <Radio size={15} aria-hidden="true" /> Start recording
          </Btn>
        </div>
      </section>

      <section className="panel">
        <h3>While it records</h3>
        <ul className="plain">
          <li>
            Segments are downloaded whole and then appended, so a dropped connection
            can never leave half a fragment in the file.
          </li>
          <li>
            The file on disk is named <code>.recording.mp4</code>. Even a hard kill
            leaves a playable video; <strong>Finalize</strong> in the Library turns it
            into a normal MP4.
          </li>
          <li>
            When the room goes offline the playlist ends, the file is closed and
            remuxed, and the job reports how much was saved.
          </li>
        </ul>
        {running ? (
          <Empty
            title={`Recording ${running.label}`}
            hint={running.message || "writing segments"}
            action={
              <Btn
                variant="danger"
                onClick={async () => {
                  await api.jobCancel(running.id);
                  reload();
                  toast("Stopping at the next segment");
                }}
              >
                <Square size={13} aria-hidden="true" /> Stop and save
              </Btn>
            }
          />
        ) : null}
      </section>
    </div>
  );
}
