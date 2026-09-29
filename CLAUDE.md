# Atlas Voice (Tauri)

Local push-to-talk dictation for macOS: hold a key, speak, release, and the transcript is pasted into the focused text field of any app. A rebuild of the Swift app in `~/sites/atlas-voice` (functional reference only, not ported).

**Scope:** personal/internal use (a Wispr Flow replacement for the owner and a few colleagues). macOS only, local models only, no cloud, no accounts, no telemetry. Don't add cloud or other platforms unless asked. Details: `docs/plan.md`.

## Working agreements
- Communicate with the user in **Dutch**. Code, comments and commit messages in **English**.
- The user is learning Rust: briefly explain a Rust concept the first time it appears, and keep it practical.
- Quality over speed. Handle errors explicitly, never store audio unless the user opts in, clean up processes on exit.
- **Name every new dependency** (and why) before adding it. Pin versions (`~x.y` or exact for RC crates).
- Record significant decisions as short ADRs in `docs/decisions/NNNN-title.md`.
- Code copied/adapted from MIT projects: keep a header comment and add it to `THIRD_PARTY_NOTICES.md`. Never copy GPL/AGPL code.
- Build in the phase order of `docs/plan.md` §5. Each phase ends testable.
- Privacy: never log transcript text, audio or device names.

## Commands
Rust lives in `~/.cargo` (the Makefile adds it to PATH; in a raw shell run `source ~/.cargo/env`).

| Command | What it does |
|---|---|
| `make install` | `npm install` |
| `make app` | **Signed** debug `.app`, launched. Use this for anything involving permissions (mic, accessibility). |
| `make dev` | `tauri dev` with hot reload. Unsigned: macOS attributes permissions to the terminal, so don't test permission flows here. |
| `make check` | fmt check, clippy (`-D warnings`), prettier, eslint, tsc, cargo test, vitest |
| `make fmt` | Auto-format Rust and TS |
| `make logs` | Tail `~/Library/Logs/nl.atlasvoice.desktop/` |
| `make reset-permissions` | `tccutil reset` for this bundle id |

Run `make check` before every commit.

## Structure
```
index.html / overlay.html  two Vite entries (main window, overlay pill)
src/                    React 19 + TS + Vite + Tailwind 4 (Radix/zustand from phase 3)
  main/                 main window: App.tsx (status cards), useAppStatus.ts
  overlay/              overlay pill (plain CSS, no Tailwind, tiny bundle)
  shared/               ipc.ts (typed commands + events, mirrors Rust types), theme.css
src-tauri/src/
  lib.rs                builder, plugins, run loop (ExitRequested / Exit / Reopen)
  services.rs           starts subsystems, holds `Services` (managed state), shutdown order
  app/                  lifecycle (quit + shutdown), windows (Dock policy), menu, tray, logging
  dictation/            state.rs = pure state machine + tests; controller.rs = actor thread
  hotkey/               handy-keys: Fn (hold) + Esc (cancel); waits for Accessibility
  audio/                cpal capture thread, rtrb ring buffer, rubato resample to 16 kHz
  transcribe/           Transcriber trait; local.rs = transcribe-cpp on an inference thread
  models/               catalog (pinned HF GGUF + sha256), streaming download
  inject/               paste: pasteboard snapshot/restore + layout-aware Cmd+V (macos/)
  overlay/              NSPanel pill (tauri-nspanel), shown/hidden per phase
  commands/             thin IPC layer, no business logic
  error.rs              AppError → serialised as { kind, message }
docs/                   research.md, plan.md, decisions/ (ADRs)
scripts/dev-app.sh      build + sign + launch debug .app
```

Data: models in `~/Library/Application Support/nl.atlasvoice.desktop/models/`, logs in `~/Library/Logs/nl.atlasvoice.desktop/`.

Events Rust → UI: `dictation-state` (Phase), `dictation-notice`, `dictation-transcript`, `model-state`, `hotkey-status`. Keep `src/shared/ipc.ts` in sync when changing these types.

## Architecture rules (see docs/plan.md §1–2 and research.md §3)
- Rust owns all state. React renders state and sends intents.
- One dictation state machine (actor thread) with a **take id** per recording; stale results are dropped; Esc cancels in every phase.
- Realtime callbacks (cpal audio, CGEventTap) only push to a lock-free buffer/channel: no locks, allocation, logging or emits.
- Heavy work never in a sync `#[tauri::command]` (those run on the main thread). Inference runs on its own thread.
- AppKit calls (CGEventPost, NSPasteboard, window geometry) run on the main thread via `run_on_main_thread`.
- **Quit only through `app::lifecycle::quit`** — it releases native resources before `app.exit(0)`. Tauri does not drop managed state on exit (tauri#14420).
- Release profile uses `panic = "unwind"` on purpose (ADR 0003). Don't change it to `abort`.

## Identity
- Bundle id `nl.atlasvoice.desktop`, product name "Atlas Voice", minimum macOS 14.
- The old Swift app uses `nl.atlasvoice.app`; keep them separate (data dir, permissions).
- Dev builds are signed with the Apple Development certificate (team C5HFB928WR) so macOS permissions survive rebuilds (ADR 0002).
