import { useEffect, useState } from "react";
import { commands, errorMessage, events, subscribeAll, type Status } from "../shared/ipc";

/** macOS has no event for "Accessibility granted", so poll while it is missing. */
const ACCESSIBILITY_POLL_MS = 1500;

export function useAppStatus() {
  const [status, setStatus] = useState<Status | null>(null);
  const [lastTranscript, setLastTranscript] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const refresh = () =>
      commands
        .getStatus()
        .then(setStatus)
        .catch((err: unknown) => setError(errorMessage(err)));
    refresh();

    const unsubscribe = subscribeAll([
      events.onPhase((phase) => setStatus((s) => (s ? { ...s, phase } : s))),
      events.onModel((modelStatus) => setStatus((s) => (s ? { ...s, modelStatus } : s))),
      events.onHotkey((hotkey) => setStatus((s) => (s ? { ...s, hotkey } : s))),
      events.onTranscript(setLastTranscript),
    ]);
    return unsubscribe;
  }, []);

  const needsAccessibility = status !== null && !status.accessibility;
  useEffect(() => {
    if (!needsAccessibility) return;
    const timer = setInterval(() => {
      commands
        .getStatus()
        .then(setStatus)
        .catch(() => {});
    }, ACCESSIBILITY_POLL_MS);
    return () => clearInterval(timer);
  }, [needsAccessibility]);

  return { status, lastTranscript, error };
}
