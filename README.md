# Atlas Voice

Local push-to-talk dictation for macOS. Hold a key, speak, release: the text appears in whatever app you were typing in. Transcription runs entirely on your Mac (NVIDIA Parakeet v3 or OpenAI Whisper); no audio leaves the device.

Built with [Tauri 2](https://tauri.app), Rust and React. Work in progress — see `docs/plan.md`.

## Development
Requirements: macOS 14+, Xcode, Rust (rustup), Node 24, an Apple Development signing certificate.

```sh
make install   # npm dependencies
make app       # build, sign and launch a debug .app
make check     # lint + tests
```

## Credits
Design and research draw on [Handy](https://github.com/cjpais/Handy) (MIT). Model licenses are listed in the app.
