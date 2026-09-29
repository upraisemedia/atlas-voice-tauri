# 0004 — Transcription runtime: transcribe-cpp with GGUF models

- Status: accepted
- Date: 2026-09-29

## Context
Two Rust runtimes can run NVIDIA Parakeet v3 locally:
- `transcribe-cpp` 0.2.4 (MIT): bindings to transcribe.cpp on ggml. One runtime for Parakeet, Whisper, Canary and Nemotron (GGUF files), with a Metal backend and native cancellation. Young project, used in production by Handy and Epicenter.
- `transcribe-rs` 0.3.11 (MIT) with ONNX Runtime via `ort` 2.0.0-rc.12: a release candidate whose prebuilt binaries need AVX2 and have no official Intel-Mac build. CoreML is reported unstable for Parakeet, so it runs on CPU.

## Spike (M4, 16 GB, one 20.5 s Dutch recording of the owner's voice)

| Variant | Warm transcribe | × realtime | RSS | Model on disk |
|---|---|---|---|---|
| Parakeet v3 GGUF Q8_0, transcribe-cpp (Metal) | 0.33–0.43 s | ~50–60× | ~900 MB | 740 MB |
| Parakeet v3 GGUF Q5_K_M, transcribe-cpp (Metal) | 0.33 s | ~62× | ~740 MB | 549 MB |
| Parakeet v3 ONNX int8, transcribe-rs (CPU) | 0.74 s | ~28× | ~1.4 GB | 478 MB |
| Whisper large-v3-turbo GGUF Q5_K_M, `language=nl` (Metal) | 1.53 s | ~13× | ~800 MB | 620 MB |

Transcripts were near-identical across Parakeet variants. All missed brand names ("Claude Code" became "Clauded/Claud/Clod Code"); Whisper got "Codex" right but dropped some sentence punctuation. ONNX produced one extra error ("eventueel" became "ik zul").

The first load of a freshly downloaded GGUF took ~16 s (cold file cache and Metal pipeline setup); later loads took 0.2–0.4 s.

Both crates built without extra setup (`cmake` required for transcribe-cpp): ~30 s and ~20 s clean release builds.

## Decision
- Use **`transcribe-cpp` =0.2.4** (default `metal` feature, static link: nothing extra to bundle or sign).
- Models as GGUF from the pinned `handy-computer/*-gguf` Hugging Face repos, verified by SHA-256:
  - Parakeet TDT 0.6B v3: **Q8_0** (740 MB), the default model
  - Whisper large-v3-turbo: **Q8_0** (886 MB)
  - Whisper large-v3: **Q5_K_M** (1.16 GB)
- After a download, warm the model once in the background and show "preparing model" instead of stalling the first dictation.
- Keep the `Transcriber` trait so an ONNX backend can be added if transcribe-cpp disappoints.

## Consequences
- One native dependency for all three models; no ONNX Runtime distribution issues.
- Pin the exact version; the API is pre-1.0 (the 0.1 → 0.2 migration changed device selection).
- Brand-name accuracy is a model limitation; the dictionary (find/replace after transcription) is the remedy, as in the Swift app.
