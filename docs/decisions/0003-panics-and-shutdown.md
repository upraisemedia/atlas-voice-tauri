# 0003 — Panic strategy and shutdown path

- Status: accepted
- Date: 2026-09-29

## Context
- Native inference code (ggml/Metal) and our own code can panic. BridgeVoice and Handy both hit crashes when quitting with a model loaded (GPU resources not released).
- Tauri 2 does not emit `ExitRequested` for a system Cmd+Q (tauri#9198) and never drops managed state on exit (tauri#14420), so `Drop` implementations cannot be relied on.
- The Tauri template sets `panic = "abort"` in the release profile, which makes `catch_unwind` useless.

## Decision
- Release profile uses **`panic = "unwind"`**. Worker threads and command boundaries wrap work in `catch_unwind`; a panicking inference engine is discarded and reloaded next time instead of taking down the app.
- There is exactly **one way to quit**: `app::lifecycle::quit`, which runs `shutdown` (idempotent) and then `app.exit(0)`. The app menu's Cmd+Q and the tray's quit item are custom items that call it; the predefined Quit item is not used.
- `RunEvent::Exit` also calls `shutdown` as a safety net (verified: an AppleEvent `quit` reaches it).
- Subsystems that own native resources register their teardown in `shutdown`, in reverse start order: stop recording → stop/join inference thread → drop model explicitly → remove event tap.

## Consequences
- Slightly larger release binary than with `abort`.
- New subsystems must add an explicit teardown step; code review should check this.
