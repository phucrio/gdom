import "./App.css";

const nextSteps = [
  "Connect a Google account with OAuth + PKCE.",
  "Create a source-to-target migration job.",
  "Scan folders, review the dry-run, then run a canary.",
] as const;

export function App() {
  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">G</span> GDOM</div>
        <span className="status"><i /> Local-first foundation</span>
      </header>
      <section className="hero" aria-labelledby="page-title">
        <p className="eyebrow">GOOGLE DRIVE OWNER MIGRATOR</p>
        <h1 id="page-title">Move ownership with an auditable plan.</h1>
        <p className="lead">This workspace is for your personal Gmail accounts only. It is ready for the account registry and mock-driven migration engine; no Google account is connected and no Drive data can be changed yet.</p>
      </section>
      <section className="overview" aria-label="Migration workspace status">
        <article className="panel primary-panel">
          <div className="panel-heading"><span>Account registry</span><span className="badge">0 connected</span></div>
          <p>OAuth credentials remain in the operating-system keychain. Account roles are selected per job, never stored on an account.</p>
          <button type="button" disabled>Connect account</button>
        </article>
        <article className="panel">
          <div className="panel-heading"><span>Migration jobs</span><span className="badge">0 drafts</span></div>
          <p>Each job has one immutable source and target after scanning begins. Only one mutation job will run at a time.</p>
        </article>
      </section>
      <section className="next-steps" aria-labelledby="next-steps-title">
        <p className="eyebrow">IMPLEMENTATION PATH</p>
        <h2 id="next-steps-title">What comes next</h2>
        <ol>{nextSteps.map((step, index) => <li key={step}><span>{index + 1}</span>{step}</li>)}</ol>
      </section>
    </main>
  );
}
