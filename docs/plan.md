# Atlas Voice (Tauri): plan, versie 2

> Status: **akkoord ("go" op 29-09-2026)**, in uitvoering. De onderbouwing zit in [`research.md`](research.md). Naam: **Atlas Voice**, bundle-ID `nl.atlasvoice.desktop` (ADR 0002).

## 0. Scope

| Onderwerp | Besluit |
|---|---|
| Doel | Een eigen Wispr Flow-vervanger voor jezelf en een paar collega's. **Geen verdienmodel, geen verkoop.** |
| Platform | **Alleen macOS** (Apple Silicon; Intel alleen als dat zonder extra werk lukt). Tauri houdt Windows en Linux mogelijk, maar we bouwen er niet voor. |
| Transcriptie | **Alleen lokaal.** Geen cloud, geen backend, geen accounts. |
| Modellen | Parakeet TDT 0.6B v3 (standaard), Whisper large-v3-turbo, Whisper large-v3 |
| AI-nabewerking | Geen (geen Apple Intelligence, geen LLM). Wel regelgebaseerde normalisatie en het woordenboek. |
| Features uit de Swift-app | Geschiedenis, statistieken, woordenboek, geluiden, kladblok |
| UI | React + TS + Vite, Tailwind + Radix, zustand |
| Telemetrie en crashrapportage | Geen. Alleen een lokaal logbestand zonder inhoud, met een knop "exporteer diagnose" voor als een collega een probleem heeft. |
| Distributie | Developer ID (je bestaande organisatie-account), genotariseerde DMG, auto-update (§6). Minimaal macOS 14. |

**Bewust níet:** cloud, Windows/Linux-code, streaming, LLM-herschrijven, snippets, sync, analytics.

**Wat we wel houden omdat het goedkoop is en later deuren openhoudt:**
- de `Transcriber`-trait: nu nodig voor twee lokale engines of modelvarianten, en later zou een cloud-backend erachter passen;
- platformcode in een eigen `macos`-module.

## 1. Uitgangspunten
1. **Rust is de bron van waarheid.** Toets, audio, state machine, modellen, plakken en instellingen zitten in Rust. React toont alleen de status en stuurt intenties door.
2. **Eén state machine per opname, met een take-ID.** Verouderde resultaten worden weggegooid, en Esc werkt in elke fase.
3. **Callbacks doen niets.** De audio-callback en de event tap sturen alleen een bericht naar een kanaal.
4. **Privacy standaard.** Audio blijft in het geheugen en wordt nooit weggeschreven. Logs bevatten geen tekst. Er is geen netwerk nodig behalve voor de modeldownload en de update-check.
5. **Netjes afsluiten is een feature.** Eén shutdown-pad ruimt opname, inferentie en model expliciet op.

## 2. Architectuur

```
┌──────────────────────── React (Vite, TS) ─────────────────────────┐
│ main window: onboarding · home/history · models · dictionary ·    │
│              scratchpad · settings                                │
│ overlay window (aparte entry, minimale bundle): pill + waveform   │
└───────────▲ events (state, levels)          │ commands (typed via tauri-specta)
            │                                 ▼
┌──────────────────────────── Rust core ────────────────────────────┐
│ app/         setup, lifecycle, shutdown, single-instance, tray    │
│ dictation/   DictationController (state machine, take-id, cancel) │
│ hotkey/      handy-keys (CGEventTap: Fn, modifier-only, release)  │
│ audio/       cpal → rtrb → rubato 16 kHz → earshot VAD → buffer   │
│ transcribe/  Transcriber trait → LocalTranscriber (inference thr.)│
│ models/      catalog, download (resume + sha256), storage         │
│ inject/      NSPasteboard snapshot · Cmd+V (CGEvent) · focus      │
│ postprocess/ normalisatie, woordenboek                            │
│ overlay/     NSPanel (tauri-nspanel), positie, zichtbaarheid      │
│ settings/    SettingsActor (atomische writes, migraties)          │
│ storage/     SQLite: history, dictionary, stats                   │
│ permissions/ microfoon, accessibility                             │
│ diagnostics/ tracing naar logbestand (geen inhoud)                │
└───────────────────────────────────────────────────────────────────┘
```

### 2.1 Threads en eigenaarschap
| Thread | Doet | Communiceert via |
|---|---|---|
| Main (Tauri/AppKit) | Vensters, tray, CGEventPost, NSPasteboard, TIS | `run_on_main_thread` |
| Hotkey (handy-keys) | Event tap, alleen `send(KeyEvent)` | kanaal naar de controller |
| cpal-callback | Samples naar de `rtrb`-ringbuffer | lock-free |
| Audio-worker | Downmix, resample, VAD, niveaus (~30 fps) | ringbuffer in, kanaal uit |
| **Controller (actor)** | State machine, beslissingen, timeouts | één inbox voor alle berichten (toets, audio, UI, inferentie) |
| Inference-worker | Model laden, transcriberen, unloaden (blocking) | kanaal met reply, `catch_unwind` |
| tokio | Modeldownloads, update-check, IPC | async |

### 2.2 De state machine
```
Idle ──press──▶ Arming ──first samples──▶ Recording ──release/stop──▶ Transcribing
                  │ release tijdens arming → onthouden, direct afronden na start
            Recording ──Esc/✕──▶ Cancelled → Idle
            Recording ──te kort/stil──▶ Idle (stil, geen fout)
            Recording ──max duur──▶ Transcribing (melding "limiet bereikt")
          Transcribing ──Esc/✕──▶ Cancelled (resultaat weggegooid)
          Transcribing ──ok──▶ PostProcess ──▶ Injecting ──▶ Idle
            Injecting ──focus niet te herstellen──▶ OnClipboard (melding) → Idle
```

**Modi:**
- **HoldOrToggle** (standaard, zoals de Swift-app): vasthouden is push-to-talk; een korte tik (< 350 ms) gevolgd door een dubbeltik vergrendelt de opname.
- **PushToTalk**
- **Toggle**

**Regels:**
- Debounce 30 ms, release-grace 50 ms. Een release telt alleen na een waargenomen press.
- Een druk tijdens de verwerking wordt genegeerd.
- Minimaal 0,3 s spraak volgens de VAD; anders stil terug naar Idle.
- Maximum 10 min (instelbaar), met een waarschuwing 30 s vooraf.
- De timeout voor transcriptie schaalt mee met de audioduur en de modelsnelheid. Annuleren werkt altijd.

**De state machine is pure Rust zonder I/O en volledig unit-getest.**

### 2.3 De `Transcriber`-abstractie
De frontend kent alleen een `ModelId` uit de catalogus.

```rust
pub struct ModelInfo {
    pub id: ModelId,                // "parakeet-tdt-0.6b-v3", "whisper-large-v3-turbo", …
    pub files: Vec<ModelFile>,      // url (pinned HF revision), size, sha256
    pub languages: LanguageSupport, // Auto (Parakeet) | Selectable (Whisper: nl/en/auto)
    pub license: License,           // shown in-app (CC-BY-4.0 attribution for Parakeet)
    pub status: ModelStatus,        // NotInstalled | Downloading{..} | Ready | Error
}

#[async_trait]
pub trait Transcriber: Send + Sync {
    fn model_id(&self) -> &ModelId;
    async fn prepare(&self) -> Result<(), TranscribeError>;   // load weights, warm up
    async fn transcribe(&self, clip: AudioClip, opts: TranscribeOptions, cancel: CancelToken)
        -> Result<Transcript, TranscribeError>;
    fn unload(&self);                                          // must be safe during shutdown
}
```

- **Eén implementatie: `LocalTranscriber`.** Die praat met de inference-thread (de runtime-keuze komt uit de spike in fase 1).
  - Tijdens inferentie gaat de engine uit de mutex. Bij een panic wordt het model opnieuw geladen.
  - Idle-unload na 5 min (instelbaar).
  - Het model wordt al geladen bij het indrukken van de toets.
  - Modelwissel is een transactie.
- **Whisper** krijgt een vaste taal (nl/en, instelbaar) en VAD-trim tegen hallucinaties. **Parakeet** herkent de taal zelf.
- **`TranscribeError`** heeft een classificatie: `Cancelled`, `Empty`, `ModelMissing`, `Timeout`, `Internal`. De UI toont per klasse een eigen, korte tekst.

### 2.4 IPC
- **Commands** (via `tauri-specta`):
  - dictatie: start, stop, cancel, `get_state`;
  - modellen: list, download (Channel), cancel, delete, select;
  - instellingen: get, update(patch);
  - permissies: check, request, `open_system_settings`;
  - `list_input_devices`, `verify_hotkey`;
  - history, dictionary en stats;
  - `export_diagnostics`.
- **Events:**
  - `dictation-state` (fase, take-ID, foutklasse);
  - `model-state`;
  - `settings-changed`;
  - `permissions-changed`.
- **Niveaus:** `emit_to("overlay")`, begrensd tot ~30 fps en alleen als het overlay zichtbaar is.
- **Capabilities:** `main` krijgt de volledige set. `overlay` krijgt alleen `listen` en `cancel_dictation`, via een app-manifest in `build.rs`.

## 3. Mappenstructuur
```
atlas-voice-tauri/
├── CLAUDE.md
├── docs/{research.md, plan.md, decisions/000N-*.md}
├── scripts/dev-app.sh              # signed debug .app bouwen en starten (TCC-proof)
├── index.html · overlay.html · vite.config.ts · package.json · tailwind
├── src/
│   ├── main/        routes/ (onboarding, home, models, dictionary, scratchpad, settings), components/, stores/
│   ├── overlay/     pill + waveform
│   └── shared/      bindings.ts (gegenereerd), ui/ (Radix-wrappers), theme/
└── src-tauri/
    ├── Cargo.toml · build.rs · tauri.conf.json · Info.plist · Entitlements.plist
    ├── capabilities/{main.json, overlay.json}
    └── src/
        ├── main.rs · lib.rs
        ├── app/ · dictation/ · hotkey/ · audio/ · transcribe/ · models/
        ├── inject/ · postprocess/ · overlay/ · settings/ · storage/
        ├── permissions.rs · diagnostics.rs · error.rs
        └── commands/                  # dunne IPC-laag naar controller-berichten
```

## 4. Gebruikersflow

### 4.1 Eerste start (ook voor collega's)
1. **Welkom:** "alles draait lokaal op je Mac".
2. **Microfoon:** toestemming vragen, met een live niveaumeter en een apparaatkeuze.
3. **Toegankelijkheid:** uitleg, knop naar het juiste paneel, en de app pollt tot de permissie er is. Pas daarna starten de toets en het plakken.
4. **Toets kiezen en testen:** standaard Fn, met een deeplink naar "Druk op 🌐 om: Niets doen". De app verifieert een echte press én release en stelt anders Ctrl+Option voor.
5. **Model:** Parakeet v3 aanbevolen, met download, voortgang, stoppen en hervatten.
6. **Proefdictaat** in de app, dan "probeer het in een andere app".
7. Naar de menubalk, met een eenmalige uitleg.

### 4.2 Dagelijks gebruik
- **Opname:** Fn vasthouden → pill verschijnt (grijs tijdens arming, kleurt bij de eerste samples) → spreken → loslaten → "verwerken" → tekst in de app die actief was bij het indrukken.
- **Hands-free:** dubbeltik. Esc of ✕ annuleert, in elke fase.
- **Menubalk-menu:**
  - status;
  - start/stop;
  - laatste dictaat kopiëren;
  - model wisselen;
  - open app;
  - instellingen;
  - stoppen (met nette opruiming).
- **Mislukt plakken:** de tekst staat op het klembord met de melding "⌘V om te plakken".
- **Updates:** melding in het menu en de instellingen, met één klik installeren en herstarten.

## 5. Bouwvolgorde

Per fase lever ik code, tests, een bijgewerkte `CLAUDE.md`, ADR's en een korte testchecklist.

### Fase 0: Fundament
- `git init` en een Tauri 2.12 + React-TS-scaffold.
- `CLAUDE.md` en de eerste ADR's (stack, scope).
- Tooling: rustfmt, clippy, eslint, prettier, tsc, `cargo test`, vitest.
- `scripts/dev-app.sh`: een gesigneerde debug-`.app` met je Developer-certificaat, zodat TCC-permissies rebuilds overleven.
- Menubalk-app zonder Dock-icoon, leeg hoofdvenster, een Quit die netjes afsluit.
- **Testbaar:** de app start in de menubalk en sluit netjes af.

### Fase 1: Minimale echte end-to-end flow ⭐
- **Spike (eerst):** Parakeet v3 via `transcribe-cpp` (GGUF) tegenover ONNX int8 via `transcribe-rs`, op ±10 opnames van jouw stem (NL en EN). Meten: snelheid, kwaliteit, geheugen, buildcomplexiteit. Het resultaat komt in een ADR.
- Fn push-to-talk (`handy-keys`), een minimale state machine met take-ID, en Esc.
- Audio: cpal → rtrb → rubato 16 kHz, in het geheugen.
- Eén lokaal model (Parakeet v3), downloaden met voortgang in een simpele UI.
- Plakken: klembord-snapshot (alle types) → tekst met Concealed/Transient-markers → layout-bewuste Cmd+V op de main thread → herstel met eigendomscheck.
- NSPanel-pill met de fases.
- Permissiecheck.
- Expliciete unload van het model bij Quit.
- **Testbaar:** Fn vasthouden in Notes, Slack, VS Code of Chrome, Nederlands spreken, loslaten, en de tekst staat er. Het klembord is daarna weer zoals het was. Quit crasht niet.

### Fase 2: Kern robuust
- **Volledige state machine** met unit tests: modi, grace, release tijdens arming, Esc overal, min/max duur, timeouts.
- **Focus:** doel vastleggen bij de press. Vóór het plakken activeren en controleren, anders naar het klembord.
- **Audio:**
  - readiness na de eerste samples;
  - earshot-VAD;
  - apparaatwissel en AirPods-HFP (stream opnieuw opbouwen);
  - microfoon lui sluiten;
  - microfoonkeuze inclusief de systeemstandaard.
- **Hotkey:**
  - tap opnieuw aanzetten in de callback;
  - Secure Input-fallback;
  - alternatieve bindingen.
- **Engine:** `catch_unwind`, idle-unload, laden bij de press, modelwissel als transactie.
- **Whisper:** large-v3-turbo en large-v3 erbij, met een vaste taal en VAD-trim.
- **Postprocess:** normalisatie en de woordenboekregels.
- **Geluiden:** buiten de opname.
- **Testbaar:** een checklist met ~25 randgevallen (research §1.9 en §3).

### Fase 3: Productschil
- Onboarding (§4.1).
- **Modelbeheer:**
  - drie modellen;
  - resume, sha256 en een stall-watchdog;
  - verwijderen;
  - schijfgebruik;
  - licentievermelding;
  - fallback naar een ander gedownload model.
- **Instellingen-UI**, met de settings-actor en migraties.
- **Geschiedenis in SQLite:** 30 dagen en 500 items, uit te zetten.
- **Overige schermen:** statistieken, woordenboek (bewerken, los van andere schakelaars) en kladblok.
- **App-gedrag:** tray-menu, starten bij inloggen, single-instance.
- **Diagnose:** logbestand en "exporteer diagnose".
- **Testbaar:** een collega komt zonder hulp van installatie naar eerste dictaat.

### Fase 4: Distributie
- Developer ID-signing en notarization, met een App Store Connect API key of een app-specifiek wachtwoord.
- Genotariseerde DMG.
- Updater (sleutelpaar met backup) en een update-UI.
- Controle "draait vanaf DMG → verplaats naar Programma's".
- Lokaal `make release`-script (bouwen, signeren, notariseren, uploaden naar GitHub Releases). GitHub Actions is gratis bij een publieke repo en kan later.
- **Testbaar:** installeren op een schone Mac, Gatekeeper accepteert de app, en een update van N naar N+1 werkt.

Daarna naar behoefte: Intel-build, streaming (Nemotron), snippets, stijl per app.

## 6. Distributie en updates voor intern gebruik
- **Signing:** je Developer ID Application-certificaat van de organisatie, hetzelfde account als Gathr. Bundle-ID `nl.atlasvoice.desktop`, los van de Swift-app (`nl.atlasvoice.app`).
- **Updates:** `tauri-plugin-updater` leest een `latest.json` die een platte, publiek leesbare URL moet hebben.
  - Bij een **private** GitHub-repo werkt "latest release" niet anoniem.
  - Opties: (a) de repo publiek maken, (b) de DMG en `latest.json` naar een publiek bucket (bijv. Cloudflare R2, gratis tier) of je eigen server, (c) geen auto-update en de DMG handmatig delen.
  - **Besloten: (a), een publieke GitHub-repo.** `latest.json` en de DMG staan als release-assets op GitHub.
- **Releases:** lokaal bouwen kan prima met één `make release` (bouwen, signeren, notariseren, uploaden). CI is optioneel: bij een publieke repo zijn GitHub Actions-minuten gratis, maar lokaal is voor een klein intern project het eenvoudigst.

## 7. Risico's
| Risico | Aanpak |
|---|---|
| `transcribe-cpp` is jong | Spike in fase 1. De trait maakt ONNX (`transcribe-rs`) inwisselbaar. Versies pinnen. |
| Crash in native inferentie | In-process met `catch_unwind` en een expliciete unload; `GGML_METAL_NO_RESIDENCY=1`. Sidecar-proces als uitwijk. |
| TCC-permissies weg na rebuild | Vaste signing-identity vanaf fase 0 |
| Fn niet betrouwbaar op sommige toetsenborden | Verificatie in de onboarding en een alternatief voorstellen |
| Klembordherstel met trage apps en klembordmanagers | Markers, eigendomscheck, instelbare vertraging, optie "niet herstellen" |
| AirPods-HFP en apparaatwissel | Stream opnieuw opbouwen, lui sluiten, testen met AirPods |
| Tauri-afsluitbugs (#9198, #14420) | Eigen quit-pad, cleanup op de main thread |
| Snel veranderende crate-API's | Exacte versies pinnen, adapters dun houden |

**Dependencies** (alle MIT/Apache):
- Tauri 2.12 en plugins (tray, updater, autostart, single-instance, log, process, deep-link niet nodig)
- `handy-keys`, `transcribe-cpp` (native ggml-build met CMake en Metal), `tauri-nspanel`
- `cpal`, `rtrb`, `rubato`, `earshot`
- `objc2`-familie
- `rusqlite`, `reqwest`, `sha2`
- `tauri-specta` (RC)
- frontend: React, zustand, Tailwind, Radix

**Mogelijk:** ONNX Runtime, als de spike daarvoor kiest.

## 8. Vervallen ten opzichte van v1
Cloud (BYOK en beheerd), Laravel-backend, betalingen, accounts, Windows, Linux, Sentry en analytics.

## 9. Besluiten (29-09-2026)
- Naam blijft **Atlas Voice**.
- Bundle-ID `nl.atlasvoice.desktop`.
- De GitHub-repo mag publiek; updates via GitHub Releases.
