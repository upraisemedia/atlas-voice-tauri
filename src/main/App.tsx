import { useEffect, useState, type ReactNode } from "react";
import {
  commands,
  errorMessage,
  type AppInfo,
  type ModelSpec,
  type ModelStatus,
  type Phase,
  type Status,
} from "../shared/ipc";
import { useAppStatus } from "./useAppStatus";

export default function App() {
  const { status, lastTranscript, error } = useAppStatus();
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    commands
      .appInfo()
      .then(setInfo)
      .catch(() => {});
  }, []);

  return (
    <main className="mx-auto flex max-w-2xl flex-col gap-4 p-8">
      <header className="mb-2">
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-muted">
          Lokaal dicteren
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Atlas Voice</h1>
      </header>

      {error && <p className="text-sm text-danger">{error}</p>}
      {status && (
        <>
          <AccessibilityCard status={status} />
          <ModelCard model={status.model} modelStatus={status.modelStatus} />
          <DictationCard status={status} lastTranscript={lastTranscript} />
        </>
      )}

      <footer className="mt-4 text-xs text-muted">
        {info ? `Versie ${info.version}` : null}
      </footer>
    </main>
  );
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-card border border-border bg-surface p-6 shadow-sm">
      <h2 className="text-sm font-semibold">{title}</h2>
      <div className="mt-3 text-sm">{children}</div>
    </section>
  );
}

function Button({
  onClick,
  children,
  variant = "primary",
}: {
  onClick: () => void;
  children: ReactNode;
  variant?: "primary" | "secondary";
}) {
  const style =
    variant === "primary"
      ? "bg-accent text-bg hover:opacity-90"
      : "border border-border hover:bg-bg";
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-control px-3 py-1.5 text-sm font-medium ${style}`}
    >
      {children}
    </button>
  );
}

function AccessibilityCard({ status }: { status: Status }) {
  if (status.accessibility) {
    return (
      <Card title="Toegankelijkheid">
        <p className="text-success">
          ✓ Toegestaan. Atlas Voice kan de Fn-toets zien en plakken.
        </p>
      </Card>
    );
  }
  return (
    <Card title="Toegankelijkheid">
      <p className="text-muted">
        Atlas Voice heeft toegankelijkheidsrechten nodig om de Fn-toets te herkennen en
        tekst in andere apps te plakken. Zet Atlas Voice aan in Systeeminstellingen →
        Privacy en beveiliging → Toegankelijkheid.
      </p>
      <div className="mt-4">
        <Button onClick={() => commands.openAccessibilitySettings()}>
          Open Systeeminstellingen
        </Button>
      </div>
    </Card>
  );
}

function formatMb(bytes: number) {
  return `${Math.round(bytes / 1_000_000)} MB`;
}

function ModelCard({
  model,
  modelStatus,
}: {
  model: ModelSpec;
  modelStatus: ModelStatus;
}) {
  const [actionError, setActionError] = useState<string | null>(null);
  const download = () => {
    setActionError(null);
    commands.downloadModel().catch((err: unknown) => setActionError(errorMessage(err)));
  };

  let body: ReactNode;
  switch (modelStatus.status) {
    case "notInstalled":
      body = <Button onClick={download}>Download ({formatMb(model.sizeBytes)})</Button>;
      break;
    case "downloading": {
      const pct = modelStatus.total
        ? (modelStatus.downloaded / modelStatus.total) * 100
        : 0;
      body = (
        <div className="flex items-center gap-3">
          <div className="h-2 flex-1 overflow-hidden rounded-full bg-bg">
            <div className="h-full bg-accent" style={{ width: `${pct}%` }} />
          </div>
          <span className="w-28 text-right font-mono text-xs text-muted">
            {formatMb(modelStatus.downloaded)} / {formatMb(modelStatus.total)}
          </span>
          <Button variant="secondary" onClick={() => commands.cancelDownload()}>
            Stop
          </Button>
        </div>
      );
      break;
    }
    case "verifying":
      body = <p className="text-muted">Download controleren…</p>;
      break;
    case "loading":
      body = (
        <p className="text-muted">
          Model voorbereiden… De eerste keer kan dit ~15 seconden duren.
        </p>
      );
      break;
    case "ready":
      body = <p className="text-success">✓ Klaar voor gebruik</p>;
      break;
    case "error":
      body = (
        <div className="flex items-center gap-3">
          <p className="flex-1 text-danger">{modelStatus.message}</p>
          <Button onClick={download}>Opnieuw</Button>
        </div>
      );
      break;
  }

  return (
    <Card title={`Model: ${model.name}`}>
      <p className="text-muted">{model.description}</p>
      <p className="mt-1 text-xs text-muted">Licentie: {model.license}</p>
      <div className="mt-4">{body}</div>
      {actionError && modelStatus.status !== "error" && (
        <p className="mt-2 text-danger">{actionError}</p>
      )}
    </Card>
  );
}

const PHASE_LABELS: Record<Phase["phase"], string> = {
  idle: "Klaar",
  arming: "Microfoon starten…",
  recording: "Luisteren…",
  transcribing: "Verwerken…",
  injecting: "Plakken…",
};

function DictationCard({
  status,
  lastTranscript,
}: {
  status: Status;
  lastTranscript: string | null;
}) {
  const ready =
    status.accessibility &&
    status.hotkey === "active" &&
    status.modelStatus.status === "ready";
  return (
    <Card title="Dicteren">
      {ready ? (
        <p>
          Houd <Kbd>fn</Kbd> ingedrukt, spreek, en laat los. De tekst verschijnt in de app
          waar je aan het typen was. <Kbd>esc</Kbd> annuleert.
        </p>
      ) : (
        <p className="text-muted">Rond de stappen hierboven af om te beginnen.</p>
      )}
      <p className="mt-3 text-xs text-muted">
        Status: {PHASE_LABELS[status.phase.phase]}
        {status.hotkey === "failed" && " · sneltoets kon niet worden geregistreerd"}
      </p>
      {lastTranscript && (
        <blockquote className="mt-4 rounded-control border border-border bg-bg p-3 select-text">
          {lastTranscript}
        </blockquote>
      )}
      <p className="mt-4 text-xs text-muted">
        Tip: zet in Systeeminstellingen → Toetsenbord "Druk op 🌐 om" op "Doe niets",
        anders opent de Fn-toets ook de emojikiezer.
      </p>
    </Card>
  );
}

function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border border-border bg-bg px-1.5 py-0.5 font-mono text-xs">
      {children}
    </kbd>
  );
}
