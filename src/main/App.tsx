import { useEffect, useState } from "react";
import { commands, isAppError, type AppInfo } from "../shared/ipc";

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    commands
      .appInfo()
      .then(setInfo)
      .catch((err: unknown) => setError(isAppError(err) ? err.message : String(err)));
  }, []);

  return (
    <main className="flex h-full items-center justify-center p-8">
      <section className="w-full max-w-md rounded-card border border-border bg-surface p-8 shadow-sm">
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-muted">
          Lokaal dicteren
        </p>
        <h1 className="mt-2 text-2xl font-semibold">Atlas Voice</h1>
        <p className="mt-4 text-sm text-muted">
          De app draait in de menubalk. Dicteren komt in de volgende fase.
        </p>
        <p className="mt-6 text-xs text-muted">
          {error ? (
            <span className="text-danger">{error}</span>
          ) : info ? (
            `Versie ${info.version}`
          ) : (
            "…"
          )}
        </p>
      </section>
    </main>
  );
}
