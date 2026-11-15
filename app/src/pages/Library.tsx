import { useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { Recording } from "../types";
import { Btn, Chip, Empty, Eyebrow, confirmDialog, fmtBytes, fmtDate } from "../ui";
import { FolderOpen, Trash2, Wand2 } from "lucide-react";

export default function LibraryPage({ snap, reload, toast, refreshKey }: PageProps) {
  const [rows, setRows] = useState<Recording[] | null>(null);

  async function load() {
    try {
      setRows(await api.libraryList());
    } catch (err) {
      toast(String(err));
      setRows([]);
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);

  const total = (rows ?? []).reduce((sum, r) => sum + r.size_bytes, 0);

  return (
    <div className="page">
      <header className="page-head">
        <Eyebrow>{snap.output_dir.split("/").pop() || "Output"}</Eyebrow>
        <h1>Library</h1>
        <p className="lede">
          Everything recorded in the output folder. {rows ? `${rows.length} file(s), ${fmtBytes(total)}.` : ""}
        </p>
      </header>

      <div className="row between">
        <Btn small onClick={() => void api.openOutputDir()}>
          <FolderOpen size={14} aria-hidden="true" /> Open folder
        </Btn>
        <input className="input" value={snap.output_dir} readOnly />
      </div>

      {rows === null ? (
        <Empty title="Reading the folder" />
      ) : rows.length === 0 ? (
        <Empty
          title="No recordings yet"
          hint="Start one on the Record page; finished files land here."
        />
      ) : (
        <div className="table">
          <div className="table-head">
            <span>File</span>
            <span>Size</span>
            <span>Recorded</span>
            <span />
          </div>
          {rows.map((row) => (
            <div className="table-row" key={row.path}>
              <span className="file" title={row.path}>
                {row.name}
                {row.in_progress ? <Chip tone="warn">unfinished</Chip> : null}
              </span>
              <span className="mono">{fmtBytes(row.size_bytes)}</span>
              <span className="mono">{fmtDate(row.modified)}</span>
              <span className="actions">
                {row.in_progress ? (
                  <Btn
                    small
                    onClick={async () => {
                      await api.libraryFinalize(row.path);
                      toast("Finalizing in the queue");
                      reload();
                    }}
                    title="Remux into a normal MP4"
                  >
                    <Wand2 size={13} aria-hidden="true" /> Finalize
                  </Btn>
                ) : null}
                <Btn small onClick={() => void api.libraryReveal(row.path)}>
                  Show
                </Btn>
                <Btn
                  small
                  variant="danger"
                  onClick={async () => {
                    if (!confirmDialog("Delete recording", `${row.name}\n\nThis cannot be undone.`))
                      return;
                    await api.libraryDelete(row.path);
                    toast(`Deleted ${row.name}`);
                    void load();
                  }}
                >
                  <Trash2 size={13} aria-hidden="true" />
                </Btn>
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
