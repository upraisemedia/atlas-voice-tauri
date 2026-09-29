import { useEffect, useState } from "react";
import { events, subscribeAll, type Notice, type Phase } from "../shared/ipc";

const NOTICE_MS = 3000;

export default function Overlay() {
  const [phase, setPhase] = useState<Phase>({ phase: "idle" });
  const [notice, setNotice] = useState<Notice | null>(null);

  useEffect(
    () =>
      subscribeAll([
        events.onPhase((next) => {
          setPhase(next);
          // A new recording replaces any leftover notice.
          if (next.phase === "arming") setNotice(null);
        }),
        events.onNotice(setNotice),
      ]),
    [],
  );

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  if (notice && phase.phase === "idle") {
    return <Pill tone="warning" dot="" label={notice.message} />;
  }

  switch (phase.phase) {
    case "arming":
      return <Pill dot="" label="Microfoon…" />;
    case "recording":
      return <Pill dot="live" label="Luisteren" />;
    case "transcribing":
      return <Pill dot="busy" label="Verwerken" />;
    case "injecting":
      return <Pill dot="busy" label="Plakken" />;
    case "idle":
      return null;
  }
}

function Pill({
  dot,
  label,
  tone,
}: {
  dot: "" | "live" | "busy";
  label: string;
  tone?: "warning";
}) {
  return (
    <div className="stage">
      <div className={`pill ${tone ?? ""}`} title={label}>
        <span className={`dot ${dot}`} />
        <span className="label">{label}</span>
      </div>
    </div>
  );
}
