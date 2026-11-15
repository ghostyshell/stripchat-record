import { useState } from "react";
import type { PageProps } from "../App";
import { api, pickers } from "../api";
import { PRESETS } from "../types";
import { Btn, Eyebrow } from "../ui";
import { FileVideo, Play } from "lucide-react";

export default function ConvertPage({ toast }: PageProps) {
  const [file, setFile] = useState("");
  const [preset, setPreset] = useState("h264_balanced");
  const [output, setOutput] = useState("");

  async function pick() {
    const chosen = await pickers.pickFile("Choose a video", ["mp4", "mkv", "ts", "webm", "mov"]);
    if (chosen) {
      setFile(chosen);
      setOutput("");
    }
  }

  async function start() {
    if (!file) return;
    try {
      await api.convertStart(file, preset, output || null);
      toast(`Converting ${file.split("/").pop()}`);
    } catch (err) {
      toast(String(err));
    }
  }

  const chosen = PRESETS.find((p) => p.id === preset);

  return (
    <div className="page">
      <header className="page-head">
        <Eyebrow>Encoding</Eyebrow>
        <h1>Convert and compress</h1>
        <p className="lede">
          Re-encode any recording with the same CRF presets the rip tools use, or remux
          to MP4 without touching a single frame.
        </p>
      </header>

      <section className="panel">
        <div className="field">
          <label>File</label>
          <div className="row">
            <input
              className="input grow"
              value={file}
              placeholder="choose a video file"
              onChange={(e) => setFile(e.target.value)}
            />
            <Btn onClick={() => void pick()}>
              <FileVideo size={15} aria-hidden="true" /> Browse
            </Btn>
          </div>
        </div>

        <div className="field">
          <label>Preset</label>
          <div className="preset-grid">
            {PRESETS.map((p) => (
              <button
                key={p.id}
                className={`preset ${preset === p.id ? "on" : ""}`}
                onClick={() => setPreset(p.id)}
                aria-pressed={preset === p.id}
              >
                <strong>{p.label}</strong>
                <span>{p.detail}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <label>Save as</label>
          <div className="row">
            <input
              className="input grow"
              value={output}
              placeholder={
                file
                  ? `${file.replace(/\.[^.]+$/, "")}.${preset}.mp4`
                  : "next to the source file"
              }
              onChange={(e) => setOutput(e.target.value)}
            />
            <Btn
              onClick={async () => {
                const path = await pickers.pickSavePath(
                  "Save converted video",
                  file ? `${file.split("/").pop()?.replace(/\.[^.]+$/, "")}.mp4` : "video.mp4",
                  ["mp4", "mkv"]
                );
                if (path) setOutput(path);
              }}
            >
              Choose
            </Btn>
          </div>
        </div>

        <div className="row between end">
          <span className="hint">{chosen ? chosen.detail : ""}</span>
          <Btn variant="primary" onClick={() => void start()} disabled={!file}>
            <Play size={15} aria-hidden="true" /> Start conversion
          </Btn>
        </div>
      </section>

      <section className="panel">
        <h3>What each preset does</h3>
        <ul className="plain">
          <li>
            <strong>Keep original</strong> is a stream copy: the container is rewritten,
            the picture is identical. Instant.
          </li>
          <li>
            <strong>H.264</strong> plays on anything, including old phones and TVs.
          </li>
          <li>
            <strong>H.265</strong> is roughly half the size at the same quality but needs
            a newer player and a slower encode.
          </li>
          <li>ffmpeg must be installed; the Settings page shows the path it will use.</li>
        </ul>
      </section>
    </div>
  );
}
