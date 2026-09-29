# 0002 — Bundle identity and development signing

- Status: accepted
- Date: 2026-09-29

## Context
macOS privacy permissions (Microphone, Accessibility, Input Monitoring) are stored by TCC against the app's *designated requirement*. For ad-hoc or unsigned builds that requirement is the code hash, which changes on every build, so granted permissions silently stop matching (Apple TN3127). The Swift app uses bundle id `nl.atlasvoice.app` and `~/Library/Application Support/Atlas Voice/`.

## Decision
- Bundle id **`nl.atlasvoice.desktop`**. A bundle id is a unique reverse-DNS string, not a domain we have to own. A new id keeps permissions, data directory and logs separate from the Swift app, so both can be installed side by side.
- Every build we run locally is signed with the **Apple Development** certificate (`scripts/dev-app.sh`, `make app`). The designated requirement becomes `identifier "nl.atlasvoice.desktop" and certificate leaf[subject.CN] = "Apple Development: …"`, which is stable across rebuilds.
- Releases will be signed with a **Developer ID Application** certificate from the same (organisation) team and notarized (phase 4).
- Hardened runtime is on; the only entitlement is `com.apple.security.device.audio-input`. Accessibility is a runtime TCC grant, not an entitlement.

## Consequences
- Permission flows must be tested with `make app`, not `tauri dev` (the unsigned dev binary's permissions are attributed to the terminal).
- Switching from the Apple Development to the Developer ID certificate changes the designated requirement: testers who used a dev build will have to grant permissions again once.
