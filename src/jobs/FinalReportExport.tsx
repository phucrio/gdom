import { useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import type { JobDto } from "../ipc/types.ts";
import { exportDestinationError } from "../dry-run/exportPath.ts";
import { jobStatusLabel } from "./status.ts";

type Props = {
  backend: Pick<BackendPort, "exportFinalReport">;
  job: JobDto;
  onAnnounce: (message: string) => void;
};

export function FinalReportExport({ backend, job, onAnnounce }: Props) {
  const [destination, setDestination] = useState(`gdom-final-report-${job.id}.txt`);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  async function exportReport() {
    if (busy) return;
    const invalid = exportDestinationError(destination) ??
      (/\.(txt|csv)$/i.test(destination.trim()) ? null : "Choose a .txt or .csv destination.");
    setError(invalid);
    setResult(null);
    if (invalid !== null) return;
    setBusy(true);
    try {
      const exported = await backend.exportFinalReport(job.id, destination.trim());
      const counts = exported.counts;
      const message = `Saved ${exported.path}. ${jobStatusLabel(exported.status)}: ${counts.total} total, ${counts.verified} verified, ${counts.failed} failed, ${counts.cancelled} cancelled, ${counts.skipped} skipped, ${counts.unfinished} unfinished.`;
      setResult(message);
      onAnnounce(message);
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "Could not export the final report.";
      setError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="dry-run-export" aria-labelledby="final-report-title">
      <h3 id="final-report-title">Final report</h3>
      <form className="final-report-form" onSubmit={(event) => { event.preventDefault(); void exportReport(); }}>
        <div className="field">
          <label htmlFor="final-report-destination">Destination path</label>
          <input id="final-report-destination" value={destination} disabled={busy}
            onChange={(event) => setDestination(event.target.value)} aria-invalid={error !== null}
            aria-describedby={`final-report-hint${error !== null ? " final-report-error" : ""}`} />
          <p id="final-report-hint" className="muted">Enter a local file path ending in .txt or .csv.</p>
        </div>
        {error !== null && <p id="final-report-error" className="error" role="alert">{error}</p>}
        <button type="submit" className="primary-button" disabled={busy}>
          {busy ? "Exporting…" : "Export final report"}
        </button>
        {result !== null && <p className="notice" role="status">{result}</p>}
      </form>
    </section>
  );
}
