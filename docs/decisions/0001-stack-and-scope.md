# 0001 — Stack and scope

- Status: accepted
- Date: 2026-09-29

## Context
Atlas Voice replaces a paid Wispr Flow subscription for the owner and a few colleagues. The first version was a Swift app with one local model. The rebuild should support several local models and stay portable in case other platforms are ever wanted.

## Decision
- **Tauri 2.12** (Rust backend) with **React 19 + TypeScript + Vite**, Tailwind 4 and (from phase 3) Radix + zustand.
- **macOS only**, minimum macOS 14. No Windows/Linux code, but platform-specific code lives in `macos` modules behind traits.
- **Local transcription only**: Parakeet TDT 0.6B v3 (default), Whisper large-v3-turbo, Whisper large-v3. Downloaded on first use.
- **Out of scope:** cloud transcription, backend, accounts, payments, telemetry, LLM post-processing, Apple Intelligence.
- Distribution: Developer ID signed + notarized DMG, auto-updates from a public GitHub repo.

## Consequences
- No server costs or accounts; everything runs offline after the model download.
- Tauri 3 is in alpha; we stay on 2.x and keep runtime-specific calls (NSPanel, private API) in one module to ease a later migration.
- A `Transcriber` trait still exists so the runtime (ggml vs ONNX) can be swapped; a cloud backend could be added later without touching the UI.
