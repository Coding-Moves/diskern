import React, { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { runAppOperation } from "../updateCoordinator.js";
import { humanBytes } from "../format.js";

/**
 * What is sitting in quarantine right now, read back from the manifest on
 * disk rather than from anything this session remembers.
 *
 * That distinction is the point. Quarantine is only "reversible" if the
 * record of where a file came from outlives the window it was moved in —
 * before the manifest existed, closing the app stranded every quarantined
 * file with a flattened filename nobody could read an original path out
 * of. So this renders whether or not a scan has been run.
 */
export default function QuarantineSection({ quarantineDir, refreshKey, onRestored }) {
  const [records, setRecords] = useState([]);
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState(null);
  const [busyPath, setBusyPath] = useState(null);
  const [purgePhase, setPurgePhase] = useState("idle"); // idle | confirming | working
  const [purgeNotice, setPurgeNotice] = useState(null);

  const reload = useCallback(async () => {
    if (!quarantineDir) return;
    try {
      setRecords(await invoke("list_quarantine", { quarantineDir }));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [quarantineDir]);

  useEffect(() => {
    reload();
  }, [reload, refreshKey]);

  async function restore(record) {
    setError(null);
    setBusyPath(record.quarantined_to);
    try {
      await runAppOperation(async () => {
        await invoke("restore_quarantined", {
          quarantineDir,
          quarantinedTo: record.quarantined_to,
        });
        onRestored(record);
        await reload();
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyPath(null);
    }
  }

  async function purge() {
    setError(null);
    setPurgePhase("working");
    try {
      await runAppOperation(async () => {
        const summary = await invoke("purge_quarantine", { quarantineDir });
        setPurgeNotice(
          `Deleted ${summary.files_removed} file${summary.files_removed === 1 ? "" : "s"}` +
            ` · ${humanBytes(summary.bytes_removed)} freed` +
            (summary.failed.length ? ` · ${summary.failed.length} could not be removed` : "")
        );
        await reload();
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setPurgePhase("idle");
    }
  }

  // Nothing quarantined and nothing to say about it: stay out of the way.
  if (records.length === 0 && !error && !purgeNotice) return null;

  return (
    <section className="verdict-group quarantine">
      <button className="group-header" onClick={() => setIsOpen(!isOpen)}>
        <span className={`chevron${isOpen ? " open" : ""}`}>▸</span>
        Quarantine
        <span className="group-meta">
          {records.length} file{records.length === 1 ? "" : "s"} · reversible
        </span>
      </button>
      <div className={`group-body${isOpen ? " open" : ""}`}>
        <div className="group-body-inner">
          <p className="quarantine-note">
            Moved here, not deleted. Restore puts a file back where it came from.
            Purge is the only thing in Diskern that deletes, and it deletes only
            what is listed here.
          </p>
          <ul className="findings">
            {records.map((r) => (
              <li className="finding" key={r.quarantined_to}>
                <span className="path">{r.original}</span>
                <span className="size">
                  {new Date(r.at_epoch * 1000).toLocaleString()}
                </span>
                <span className="row-action">
                  {busyPath === r.quarantined_to ? (
                    <span className="working">Restoring…</span>
                  ) : (
                    <button className="quarantine-btn" onClick={() => restore(r)}>
                      Restore
                    </button>
                  )}
                </span>
              </li>
            ))}
          </ul>

          {records.length > 0 && (
            <div className="purge">
              {purgePhase === "idle" && (
                <button className="cancel-btn" onClick={() => setPurgePhase("confirming")}>
                  Purge quarantine
                </button>
              )}
              {purgePhase === "confirming" && (
                <span className="confirm">
                  <span className="confirm-q">
                    Delete {records.length} file{records.length === 1 ? "" : "s"} for good?
                    This cannot be undone.
                  </span>
                  <button className="quarantine-btn confirm-yes" onClick={purge}>
                    Delete
                  </button>
                  <button className="confirm-no" onClick={() => setPurgePhase("idle")}>
                    Cancel
                  </button>
                </span>
              )}
              {purgePhase === "working" && <span className="working">Deleting…</span>}
            </div>
          )}

          {purgeNotice && <p className="notice">{purgeNotice}</p>}
          {error && <p className="error">{error}</p>}
        </div>
      </div>
    </section>
  );
}

