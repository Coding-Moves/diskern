import React, {
  useState,
  useMemo,
  useRef,
  useEffect,
  useCallback,
  useSyncExternalStore,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { appLocalDataDir, join } from "@tauri-apps/api/path";
import { visibleDuplicateSets } from "./duplicates.js";
import {
  runAppOperation,
  getUpdateStatus,
  subscribeUpdateStatus,
  setUpdateStatus,
} from "./updateCoordinator.js";
import { humanBytes } from "./format.js";
import BrandMark from "./BrandMark.jsx";
import CappedList from "./components/CappedList.jsx";
import ScanningIndicator from "./components/ScanningIndicator.jsx";
import UpdateStatus from "./components/UpdateStatus.jsx";

const CATEGORY_LABEL = {
  browser_cache: "Browser cache",
  build_artifact: "Build artifacts",
  package_manager_cache: "Package manager cache",
  temp_file: "Temporary files",
  log: "Logs",
  installer: "Old installers",
  system_critical: "System critical",
  unknown: "Unrecognized",
};

// Verdicts the UI is allowed to offer a quarantine action for. Risky and
// Protected are intentionally absent — the button never renders for them,
// and the Rust command re-checks server-side anyway (defense in depth).
const ACTIONABLE_VERDICTS = new Set(["safe", "review"]);

// First page a long list mounts; the rest stays out of the DOM behind a
// Show more / Show less toggle. A big report can carry thousands of
// findings, and mounting every row up front is what makes scrolling and
// section expands heavy — the cap is the cheap fix the issue asks for.
// Paths cap lower: they are tiny nodes, but a hardlink-heavy duplicate
// set can hold hundreds of them.
const FINDINGS_CAP = 100;
const DUP_SETS_CAP = 100;
const DUP_PATHS_CAP = 25;

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
function FindingRow({ f, quarantineDir, onQuarantined, actionsDisabled = false }) {
  const [phase, setPhase] = useState("idle"); // idle | confirming | working
  const [rowError, setRowError] = useState(null);

  const canQuarantine = ACTIONABLE_VERDICTS.has(f.verdict) && !actionsDisabled;

  async function doQuarantine() {
    setRowError(null);
    if (!quarantineDir) {
      setRowError("Quarantine folder isn't ready yet — try again in a moment.");
      setPhase("idle");
      return;
    }
    setPhase("working");
    try {
      await runAppOperation(async () => {
        await invoke("quarantine_finding", {
          path: f.entry.path,
          quarantineDir,
        });
        // Success: tell the parent to remove this row and update the total.
        onQuarantined(f);
      });
    } catch (e) {
      // Backend refused (e.g. re-classification changed the verdict) or the
      // move failed. Show it right here, next to the row.
      setRowError(String(e));
      setPhase("idle");
    }
  }

  return (
    <li className={`finding verdict-${f.verdict}`}>
      <span className="path">
        {f.entry.path}
        {f.category && (
          <span className="category-badge">
            {CATEGORY_LABEL[f.category] ?? f.category}
          </span>
        )}
      </span>
      <span className="size">{humanBytes(f.entry.size)}</span>
      {/* Every reason, not just the matched rule: "referenced by 3
          projects" is what explains a risky row, and it is never the
          first one. */}
      <span className="why">{f.reasons.join(" · ")}</span>

      {actionsDisabled && ACTIONABLE_VERDICTS.has(f.verdict) && (
        <span className="row-action preview-only">Preview only</span>
      )}

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

function CategorySection({ title, items, defaultOpen, quarantineDir, onQuarantined, actionsDisabled = false }) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  if (items.length === 0) return null;
  const total = items.reduce((s, f) => s + f.reclaimable, 0);

  return (
    <section className="verdict-group">
      <button className="group-header" onClick={() => setIsOpen(!isOpen)}>
        <span className={`chevron${isOpen ? " open" : ""}`}>▸</span>
        {title}
        <span className="group-meta">
          {items.length} item{items.length === 1 ? "" : "s"} · {humanBytes(total)}
        </span>
      </button>
      {/* The body stays mounted when closed so CSS can animate the
          collapse; visibility:hidden keeps it out of the tab order. */}
      <div className={`group-body${isOpen ? " open" : ""}`}>
        <div className="group-body-inner">
          {byCategory(items).map(([cat, catItems]) => (
            <div key={cat} className="category-block">
              <h4>{CATEGORY_LABEL[cat] ?? cat}</h4>
              <CappedList
                className="findings"
                items={catItems}
                cap={FINDINGS_CAP}
                renderItem={(f) => (
                  <FindingRow
                    key={f.entry.path}
                    f={f}
                    quarantineDir={quarantineDir}
                    onQuarantined={onQuarantined}
                    actionsDisabled={actionsDisabled}
                  />
                )}
              />
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/**
 * Compact summary showing top categories ranked by reclaimable bytes.
 * Gives users an immediate overview of where space is consumed
 * before they dive into individual expandable sections.
 */
function TopCategoriesSummary({ items, cap = 5 }) {
  if (!items || items.length === 0) return null;

  const categories = byCategory(items)
    .map(([cat, catItems]) => ({
      key: cat,
      label: CATEGORY_LABEL[cat] ?? cat,
      total: catItems.reduce((s, f) => s + f.reclaimable, 0),
    }))
    .filter((c) => c.total > 0)
    .slice(0, cap);

  if (categories.length === 0) return null;

  return (
    <div className="top-categories-summary" aria-label="Top categories">
      <h3 className="top-categories-title">Top categories</h3>
      <ul className="top-categories-list">
        {categories.map((c) => (
          <li key={c.key} className="top-category-row">
            <span className="top-category-label">{c.label}</span>
            <span className="top-category-size">{humanBytes(c.total)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function DuplicatesSection({ sets }) {
  const [isOpen, setIsOpen] = useState(true);
  if (sets.length === 0) return null;
  const total = sets.reduce((s, d) => s + d.wasted, 0);

  return (
    <section className="verdict-group duplicates">
      <button className="group-header" onClick={() => setIsOpen(!isOpen)}>
        <span className={`chevron${isOpen ? " open" : ""}`}>▸</span>
        Duplicate files
        <span className="group-meta">
          {sets.length} set{sets.length === 1 ? "" : "s"} · {humanBytes(total)} wasted
        </span>
      </button>
      <div className={`group-body${isOpen ? " open" : ""}`}>
        <div className="group-body-inner">
          <CappedList
            tag="div"
            className="dup-sets"
            items={sets}
            cap={DUP_SETS_CAP}
            renderItem={(set, i) => (
              <div key={i} className="dup-set">
                <div className="dup-set-header">
                  {set.paths.length} copies · {humanBytes(set.size)} each ·{" "}
                  {humanBytes(set.wasted)} wasted
                </div>
                <CappedList
                  className="dup-paths"
                  items={set.paths}
                  cap={DUP_PATHS_CAP}
                  renderItem={(p, j) => <li key={j}>{p}</li>}
                />
              </div>
            )}
          />
        </div>
      </div>
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

export default function App() {
  const [report, setReport] = useState(null);
  const [previewReport, setPreviewReport] = useState(null);
  const [scannedFolder, setScannedFolder] = useState(null);
  const [scanning, setScanning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState(null);
  // Set when a scan ends because the user stopped it. Not an error — it
  // renders as a plain note, and any previous report stays on screen.
  const [notice, setNotice] = useState(null);
  const [liveProgress, setLiveProgress] = useState({
    files_seen: 0,
    bytes_seen: 0,
    phase: "Walking files",
  });
  // Paths already quarantined this session, and the bytes they accounted for.
  // Rows in this set are filtered out of the view; reclaimed is subtracted
  // from the headline total.
  const [quarantinedPaths, setQuarantinedPaths] = useState(() => new Set());
  const [reclaimed, setReclaimed] = useState(0);
  const [quarantineDir, setQuarantineDir] = useState(null);
  // Bumped whenever this session moves a file in, so the quarantine list
  // re-reads the manifest rather than guessing at what changed.
  const [quarantineVersion, setQuarantineVersion] = useState(0);
  // The updater runs outside React — its phases live in the coordinator's
  // little store, so subscribing here (not props) is what keeps it simple.
  const updateStatus = useSyncExternalStore(
    subscribeUpdateStatus,
    getUpdateStatus
  );
  const progressUnlistenRef = useRef(null);
  const previewUnlistenRef = useRef(null);

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

  const displayedReport = report ?? previewReport;
  const showingPreview = Boolean(previewReport && !report);

  const visibleFindings = useMemo(() => {
    if (!displayedReport) return [];
    if (quarantinedPaths.size === 0) return displayedReport.findings;
    return displayedReport.findings.filter((f) => !quarantinedPaths.has(f.entry.path));
  }, [displayedReport, quarantinedPaths]);

  const groups = useMemo(
    () => (displayedReport ? groupFindings(visibleFindings) : null),
    [displayedReport, visibleFindings]
  );

  const duplicateSets = useMemo(
    () =>
      displayedReport
        ? visibleDuplicateSets(displayedReport.duplicate_sets, quarantinedPaths)
        : [],
    [displayedReport, quarantinedPaths]
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

    try {
      await runAppOperation(async () => {
        // Starting a scan invalidates the previous backend report authority.
        // Clear the view at the same time so no stale row remains actionable.
        setReport(null);
        setPreviewReport(null);
        setScannedFolder(null);
        setQuarantinedPaths(new Set());
        setReclaimed(0);
        setLiveProgress({ files_seen: 0, bytes_seen: 0, phase: "Walking files" });
        setScanning(true);
        setCancelling(false);

        try {
          // Subscribe to live progress events emitted by the Rust command
          // roughly every 150ms while the scan runs.
          progressUnlistenRef.current = await listen("scan-progress", (event) => {
            setLiveProgress(event.payload);
          });
          previewUnlistenRef.current = await listen("scan-preview", (event) => {
            const payload = event.payload;
            setPreviewReport((prev) => {
              const byPath = new Map((prev?.findings ?? []).map((f) => [f.entry.path, f]));
              for (const finding of payload.findings) {
                byPath.set(finding.entry.path, finding);
              }
              return {
                findings: [...byPath.values()],
                duplicate_sets: [],
                total_reclaimable: payload.total_reclaimable,
                files_scanned: payload.files_scanned,
              };
            });
          });

          const result = await invoke("start_scan", { roots: [folder] });
          // null means the scan was cancelled. The backend has no completed
          // report authority after a cancellation, and the view was cleared when
          // this scan began.
          if (result === null) {
            setPreviewReport(null);
            setNotice("Scan cancelled. Scanning is read-only — nothing was moved or deleted.");
          } else {
            setPreviewReport(null);
            setReport(result);
            setScannedFolder(folder);
          }
        } catch (e) {
          setError(String(e));
        } finally {
          setScanning(false);
          setCancelling(false);
          if (progressUnlistenRef.current) {
            progressUnlistenRef.current();
            progressUnlistenRef.current = null;
          }
          if (previewUnlistenRef.current) {
            previewUnlistenRef.current();
            previewUnlistenRef.current = null;
          }
        }
      });
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main className="shell">
      <header>
        <div className="brand">
          <BrandMark className="header-mark" />
          <h1>Diskern</h1>
        </div>
        <p className="tagline">Understand your disk before you clean it.</p>
      </header>

      {/* One persistent live region for scan milestones. It mounts empty
          and sits outside the report branches: ScanningIndicator only
          appears once a scan starts (already populated — polite regions
          announce changes, not initial content) and is replaced when the
          first preview swaps the view, so a region inside it could miss
          the first phase entirely. Only the phase is announced — the
          ~150ms file/byte counters stay out of it. */}
      <p className="sr-only" aria-live="polite" aria-atomic="true">
        {scanning
          ? cancelling
            ? "Stopping the scan… nothing has been changed"
            : liveProgress.phase || "Walking files"
          : ""}
      </p>

      {!displayedReport && (
        <section className="empty">
          <p> Run a read-only scan of Downloads, a project folder, or another folder you want to understand.
              Nothing is deleted — ever — without your review.</p>
          <button onClick={runScan} disabled={scanning}>
            {scanning ? "Scanning…" : "Choose a folder to scan"}
          </button>
          {scanning && (
            <ScanningIndicator
              filesSeen={liveProgress.files_seen}
              bytesSeen={liveProgress.bytes_seen}
              phase={liveProgress.phase}
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

      {displayedReport && (
        <section>
          <p className="summary">
            {showingPreview ? (
              <span className="scanned-folder">Preview while scanning</span>
            ) : (
              scannedFolder && <span className="scanned-folder">{scannedFolder}</span>
            )}
            <br />
            {displayedReport.files_scanned.toLocaleString()} files scanned ·{" "}
            {humanBytes(displayedReport.total_reclaimable - reclaimed)} reclaimable
          </p>
          {showingPreview && (
            <p className="preview-note">
              Early results are appearing now. Final safety checks and actions unlock when
              the scan finishes.
            </p>
          )}

          <button onClick={runScan} disabled={scanning}>
            {scanning ? "Scanning…" : "Scan a different folder"}
          </button>
          {scanning && (
            <ScanningIndicator
              filesSeen={liveProgress.files_seen}
              bytesSeen={liveProgress.bytes_seen}
              phase={liveProgress.phase}
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

          <TopCategoriesSummary items={visibleFindings} />

          <DuplicatesSection sets={duplicateSets} />
          <CategorySection
            title="Safe to quarantine"
            items={groups.safe}
            defaultOpen={true}
            quarantineDir={quarantineDir}
            onQuarantined={handleQuarantined}
            actionsDisabled={showingPreview}
          />
          <CategorySection
            title="Review first"
            items={groups.review}
            defaultOpen={true}
            quarantineDir={quarantineDir}
            onQuarantined={handleQuarantined}
            actionsDisabled={showingPreview}
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

      {/* Fixed corner toast — renders over either view and never takes
          part in the layout either one is managing. */}
      <UpdateStatus
        status={updateStatus}
        onDismiss={() => setUpdateStatus(null)}
      />
    </main>
  );
}
