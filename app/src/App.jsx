import React, { useState, useMemo, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { appLocalDataDir, join } from "@tauri-apps/api/path";
import { appState } from "./updater.js";

const CATEGORY_LABEL = {
  browser_cache: "Browser cache",
  build_artifact: "Build artifacts",
  package_manager_cache: "Package manager cache",
  temp_file: "Temporary files",
  log: "Logs",
  installer: "Old installers",
  duplicate_file: "Duplicate file",
  empty_directory: "Empty folders",
  system_critical: "System critical",
  unknown: "Unrecognized",
};

// Verdicts the UI is allowed to offer a quarantine action for. Risky and
// Protected are intentionally absent — the button never renders for them,
// and the Rust command re-checks server-side anyway (defense in depth).
const ACTIONABLE_VERDICTS = new Set(["safe", "review"]);

function bytesToGB(n) {
  return (n / 1e9).toFixed(2);
}

function groupFindings(findings) {
  const groups = { safe: [], review: [], risky: [], protected: [] };
  for (const f of findings) {
    (groups[f.verdict] ?? groups.review).push(f);
  }
  return groups;
}

function byCategory(items) {
  const map = new Map();
  for (const f of items) {
    const key = f.category;
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(f);
  }
  return [...map.entries()].sort(
    (a, b) =>
      b[1].reduce((s, f) => s + f.reclaimable, 0) -
      a[1].reduce((s, f) => s + f.reclaimable, 0)
  );
}

/**
 * One finding row. Owns its own quarantine interaction state (confirm →
 * working → error) so a failure on one row surfaces inline next to that
 * row, never as a global alert. On success it calls onQuarantined so the
 * parent can drop the row and adjust the running total.
 *
 * The action button only renders for safe/review verdicts. The parent
 * never even passes risky/protected rows a usable quarantineDir path, but
 * the verdict gate here is the visible guarantee the task requires.
 */
function FindingRow({ f, quarantineDir, onQuarantined }) {
  const [phase, setPhase] = useState("idle"); // idle | confirming | working
  const [rowError, setRowError] = useState(null);

  const canQuarantine = ACTIONABLE_VERDICTS.has(f.verdict);

  async function doQuarantine() {
    setRowError(null);
    if (!quarantineDir) {
      setRowError("Quarantine folder isn't ready yet — try again in a moment.");
      setPhase("idle");
      return;
    }
    setPhase("working");
    try {
      await invoke("quarantine_finding", {
        path: f.entry.path,
        quarantineDir,
      });
      // Success: tell the parent to remove this row and update the total.
      onQuarantined(f);
    } catch (e) {
      // Backend refused (e.g. re-classification changed the verdict) or the
      // move failed. Show it right here, next to the row.
      setRowError(String(e));
      setPhase("idle");
    }
  }

  return (
    <li className={`finding verdict-${f.verdict}`}>
      <span className="path">{f.entry.path}</span>
      <span className="size">{(f.entry.size / 1e6).toFixed(1)} MB</span>
      <span className="why">{f.reasons[0]}</span>

      {canQuarantine && (
        <span className="row-action">
          {phase === "idle" && (
            <button
              className="quarantine-btn"
              onClick={() => {
                setRowError(null);
                setPhase("confirming");
              }}
            >
              Quarantine
            </button>
          )}
          {phase === "confirming" && (
            <span className="confirm">
              <span className="confirm-q">Move to quarantine?</span>
              <button className="quarantine-btn confirm-yes" onClick={doQuarantine}>
                Yes
              </button>
              <button className="confirm-no" onClick={() => setPhase("idle")}>
                Cancel
              </button>
            </span>
          )}
          {phase === "working" && <span className="working">Moving…</span>}
        </span>
      )}

      {rowError && <span className="row-error">{rowError}</span>}
    </li>
  );
}

function CategorySection({ title, items, defaultOpen, quarantineDir, onQuarantined }) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  if (items.length === 0) return null;
  const total = items.reduce((s, f) => s + f.reclaimable, 0);

  return (
    <section className="verdict-group">
      <button className="group-header" onClick={() => setIsOpen(!isOpen)}>
        <span className="chevron">{isOpen ? "▾" : "▸"}</span>
        {title}
        <span className="group-meta">
          {items.length} item{items.length === 1 ? "" : "s"} · {bytesToGB(total)} GB
        </span>
      </button>
      {isOpen && (
        <div className="group-body">
          {byCategory(items).map(([cat, catItems]) => (
            <div key={cat} className="category-block">
              <h4>{CATEGORY_LABEL[cat] ?? cat}</h4>
              <ul className="findings">
                {catItems.map((f) => (
                  <FindingRow
                    key={f.entry.path}
                    f={f}
                    quarantineDir={quarantineDir}
                    onQuarantined={onQuarantined}
                  />
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function DuplicatesSection({ sets }) {
  const [isOpen, setIsOpen] = useState(true);
  if (sets.length === 0) return null;
  const total = sets.reduce((s, d) => s + d.wasted, 0);

  return (
    <section className="verdict-group duplicates">
      <button className="group-header" onClick={() => setIsOpen(!isOpen)}>
        <span className="chevron">{isOpen ? "▾" : "▸"}</span>
        Duplicate files
        <span className="group-meta">
          {sets.length} set{sets.length === 1 ? "" : "s"} · {bytesToGB(total)} GB wasted
        </span>
      </button>
      {isOpen && (
        <div className="group-body">
          {sets.map((set, i) => (
            <div key={i} className="dup-set">
              <div className="dup-set-header">
                {set.paths.length} copies · {(set.size / 1e6).toFixed(1)} MB each ·{" "}
                {(set.wasted / 1e6).toFixed(1)} MB wasted
              </div>
              <ul className="dup-paths">
                {set.paths.map((p, j) => (
                  <li key={j}>{p}</li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

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
function QuarantineSection({ quarantineDir, refreshKey, onRestored }) {
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
      await invoke("restore_quarantined", {
        quarantineDir,
        quarantinedTo: record.quarantined_to,
      });
      onRestored(record);
      await reload();
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
      const summary = await invoke("purge_quarantine", { quarantineDir });
      setPurgeNotice(
        `Deleted ${summary.files_removed} file${summary.files_removed === 1 ? "" : "s"}` +
          ` · ${(summary.bytes_removed / 1e6).toFixed(1)} MB freed` +
          (summary.failed.length ? ` · ${summary.failed.length} could not be removed` : "")
      );
      await reload();
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
        <span className="chevron">{isOpen ? "▾" : "▸"}</span>
        Quarantine
        <span className="group-meta">
          {records.length} file{records.length === 1 ? "" : "s"} · reversible
        </span>
      </button>
      {isOpen && (
        <div className="group-body">
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
      )}
    </section>
  );
}

/**
 * Live "is this actually working" feedback while a scan runs.
 *
 * There's no true percentage to show — the total file count on disk isn't
 * known until the walk finishes, so faking a 0-100% number would just be
 * lying with more decimals. Instead: a real, continuously-updating count
 * of files found so far (proof of life) plus an animated indeterminate
 * bar (motion reads as "working," not "frozen").
 */
function ScanningIndicator({ filesSeen, bytesSeen, onCancel, cancelling }) {
  return (
    <div className="scan-progress">
      <div className="progress-track">
        <div className="progress-fill-indeterminate" />
      </div>
      <p className="progress-count">
        {filesSeen.toLocaleString()} files found
        {bytesSeen > 0 && <> · {(bytesSeen / 1e9).toFixed(2)} GB so far</>}
      </p>
      {/* The walk checks the cancel flag per entry, so stopping is quick but
          not instant — say "Stopping…" rather than pretending it's done. */}
      <button className="cancel-btn" onClick={onCancel} disabled={cancelling}>
        {cancelling ? "Stopping…" : "Cancel scan"}
      </button>
    </div>
  );
}

export default function App() {
  const [report, setReport] = useState(null);
  const [scannedFolder, setScannedFolder] = useState(null);
  const [scanning, setScanning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState(null);
  // Set when a scan ends because the user stopped it. Not an error — it
  // renders as a plain note, and any previous report stays on screen.
  const [notice, setNotice] = useState(null);
  const [liveProgress, setLiveProgress] = useState({ files_seen: 0, bytes_seen: 0 });
  // Paths already quarantined this session, and the bytes they accounted for.
  // Rows in this set are filtered out of the view; reclaimed is subtracted
  // from the headline total.
  const [quarantinedPaths, setQuarantinedPaths] = useState(() => new Set());
  const [reclaimed, setReclaimed] = useState(0);
  const [quarantineDir, setQuarantineDir] = useState(null);
  // Bumped whenever this session moves a file in, so the quarantine list
  // re-reads the manifest rather than guessing at what changed.
  const [quarantineVersion, setQuarantineVersion] = useState(0);
  const unlistenRef = useRef(null);

  // Resolve a sensible, always-writable quarantine location once on mount:
  // <app local data dir>/Quarantine (e.g. %LOCALAPPDATA%\com.diskern.app\
  // Quarantine on Windows). The Rust side create_dir_all's it, so it doesn't
  // need to pre-exist.
  useEffect(() => {
    (async () => {
      try {
        const base = await appLocalDataDir();
        setQuarantineDir(await join(base, "Quarantine"));
      } catch {
        // Leave null; the row handler surfaces a friendly error if a user
        // clicks Quarantine before this resolves (or if it failed).
      }
    })();
  }, []);

  const visibleFindings = useMemo(() => {
    if (!report) return [];
    if (quarantinedPaths.size === 0) return report.findings;
    return report.findings.filter((f) => !quarantinedPaths.has(f.entry.path));
  }, [report, quarantinedPaths]);

  const groups = useMemo(
    () => (report ? groupFindings(visibleFindings) : null),
    [report, visibleFindings]
  );

  function handleQuarantined(finding) {
    setQuarantinedPaths((prev) => {
      const next = new Set(prev);
      next.add(finding.entry.path);
      return next;
    });
    setReclaimed((prev) => prev + finding.reclaimable);
    setQuarantineVersion((v) => v + 1);
  }

  // A restored file is back on disk, so it belongs back in the report it
  // came from — as a row again, and out of the reclaimed running total.
  // A record with no matching finding (restored after a different scan, or
  // after a restart) just isn't in this report; the list still refreshes.
  function handleRestored(record) {
    const finding = report?.findings.find((f) => f.entry.path === record.original);
    setQuarantinedPaths((prev) => {
      if (!prev.has(record.original)) return prev;
      const next = new Set(prev);
      next.delete(record.original);
      return next;
    });
    if (finding) setReclaimed((prev) => Math.max(0, prev - finding.reclaimable));
  }

  // Cancelling races the scan finishing on its own; the command returns
  // false in that case and there's nothing to report either way.
  async function cancelScan() {
    setCancelling(true);
    try {
      await invoke("cancel_scan");
    } catch (e) {
      setError(String(e));
      setCancelling(false);
    }
  }

  async function runScan() {
    setError(null);
    setNotice(null);

    let folder;
    try {
      folder = await open({ directory: true, multiple: false });
    } catch (e) {
      setError(`Couldn't open the folder picker: ${e}`);
      return;
    }
    if (!folder) return;

    setLiveProgress({ files_seen: 0, bytes_seen: 0 });
    setScanning(true);
    setCancelling(false);
    appState.busy = true;

    // Subscribe to live progress events emitted by the Rust command
    // roughly every 150ms while the scan runs.
    unlistenRef.current = await listen("scan-progress", (event) => {
      setLiveProgress(event.payload);
    });

    try {
      const result = await invoke("start_scan", { roots: [folder] });
      // null means the scan was cancelled. Keep whatever report was already
      // on screen rather than blanking the view.
      if (result === null) {
        setNotice("Scan cancelled. Scanning is read-only — nothing was moved or deleted.");
      } else {
        setReport(result);
        setScannedFolder(folder);
        // Fresh scan — clear any prior session's quarantine bookkeeping.
        setQuarantinedPaths(new Set());
        setReclaimed(0);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
      setCancelling(false);
      appState.busy = false;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    }
  }

  return (
    <main className="shell">
      <header>
        <h1>Diskern</h1>
        <p className="tagline">Understand your disk before you clean it.</p>
      </header>

      {!report && (
        <section className="empty">
          <p>Run a read-only scan. Nothing is deleted — ever — without your review.</p>
          <button onClick={runScan} disabled={scanning}>
            {scanning ? "Scanning…" : "Choose a folder to scan"}
          </button>
          {scanning && (
            <ScanningIndicator
              filesSeen={liveProgress.files_seen}
              bytesSeen={liveProgress.bytes_seen}
              onCancel={cancelScan}
              cancelling={cancelling}
            />
          )}
          {error && <p className="error">{error}</p>}
          {notice && <p className="notice">{notice}</p>}
          {/* Rendered before any scan too: files quarantined in an earlier
              session are restorable without scanning again. */}
          <QuarantineSection
            quarantineDir={quarantineDir}
            refreshKey={quarantineVersion}
            onRestored={handleRestored}
          />
        </section>
      )}

      {report && (
        <section>
          <p className="summary">
            {scannedFolder && <span className="scanned-folder">{scannedFolder}</span>}
            <br />
            {report.files_scanned.toLocaleString()} files scanned ·{" "}
            {bytesToGB(report.total_reclaimable - reclaimed)} GB reclaimable
          </p>

          <button onClick={runScan} disabled={scanning}>
            {scanning ? "Scanning…" : "Scan a different folder"}
          </button>
          {scanning && (
            <ScanningIndicator
              filesSeen={liveProgress.files_seen}
              bytesSeen={liveProgress.bytes_seen}
              onCancel={cancelScan}
              cancelling={cancelling}
            />
          )}
          {error && <p className="error">{error}</p>}
          {notice && <p className="notice">{notice}</p>}

          <QuarantineSection
            quarantineDir={quarantineDir}
            refreshKey={quarantineVersion}
            onRestored={handleRestored}
          />

          <DuplicatesSection sets={report.duplicate_sets} />
          <CategorySection
            title="Safe to remove"
            items={groups.safe}
            defaultOpen={true}
            quarantineDir={quarantineDir}
            onQuarantined={handleQuarantined}
          />
          <CategorySection
            title="Review first"
            items={groups.review}
            defaultOpen={true}
            quarantineDir={quarantineDir}
            onQuarantined={handleQuarantined}
          />
          <CategorySection
            title="Risky — not recommended"
            items={groups.risky}
            defaultOpen={false}
          />
          <CategorySection
            title="Protected — do not touch"
            items={groups.protected}
            defaultOpen={false}
          />
        </section>
      )}
    </main>
  );
}
