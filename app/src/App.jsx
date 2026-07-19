import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { appState } from "./updater.js";

const VERDICT_LABEL = {
  safe: "Safe to remove",
  review: "Review first",
  risky: "Risky",
  protected: "Protected",
};

export default function App() {
  const [report, setReport] = useState(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState(null);

  async function runScan() {
    setScanning(true);
    setError(null);
    appState.busy = true;
    try {
      // TODO: folder picker via @tauri-apps/plugin-dialog; home dir for now.
      const result = await invoke("start_scan", { roots: ["."] });
      setReport(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
      appState.busy = false;
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
            {scanning ? "Scanning…" : "Scan my files"}
          </button>
          {error && <p className="error">{error}</p>}
        </section>
      )}

      {report && (
        <section>
          <p className="summary">
            {report.files_scanned.toLocaleString()} files scanned ·{" "}
            {(report.total_reclaimable / 1e9).toFixed(2)} GB reclaimable
          </p>
          <ul className="findings">
            {report.findings.map((f, i) => (
              <li key={i} className={`verdict-${f.verdict}`}>
                <span className="path">{f.entry.path}</span>
                <span className="badge">{VERDICT_LABEL[f.verdict]}</span>
                <span className="why">{f.reasons[0]}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}
