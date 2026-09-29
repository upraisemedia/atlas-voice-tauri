# Bijlage: BridgeVoice-changelog (v2.2.50 – v4.1.18)

Deze bijlage hoort bij `research.md` §2–3. Bron: https://www.bridgemind.ai/changelog/bridgevoice/v… Opgehaald via de browser op 2026-09-29, omdat de site achter een Cloudflare-challenge zit.

De publieke changelog begint bij v2.2.50 (13 mei 2026). Versienummers die ontbreken, zijn niet gepubliceerd. De items zijn samengevat; citaten staan tussen aanhalingstekens.

| Versie | Datum | Belangrijkste punten |
|---|---|---|
| 4.1.18 | 28 sep | Linux AppImage gaf een wit venster met recente Mesa, omdat een oude `libwayland-client` meegebundeld werd. Het X11-gedrag van de widget blijft; te overschrijven via `GDK_BACKEND`. |
| 4.1.17 | 25 sep | Browser-login herstelbaar na een afgewezen callback (tot 5 min wachten). **"Avoid falling back to the clipboard when the final macOS focus-restoration attempt successfully reaches the original text field."** App-identiteit geregistreerd bij de Wayland-portal. Diagnostiek zonder tekst, audio of apparaatnamen. |
| 4.1.15 | 22 sep | Parakeet Unified (EN, 663 MB). Interpunctie door het model zelf, niet op Intel-Macs. |
| 4.1.14 | 22 sep | Tolerante migratie van opgeslagen sessies. Crash in ongebruikte native file-drop opgelost. Originele microfoonfout tonen. Storing, limiet en credits apart onderscheiden. |
| 4.1.13 | 15 sep | Zachte spraak: geen harde volumedrempel meer, zachte woorden rond luide spraak blijven behouden. Bijna-stille opnames en klikken nog steeds overslaan. |
| 4.1.12 | 14 sep | **Microfoon start parallel aan het opzoeken van het doelveld en de voorbereiding van het woordenboek**, waarmee de startvertraging op macOS weg is. Fn-release blijft responsief tijdens de voorbereiding. TLS-lib geüpdatet. |
| 4.1.10 | 13 sep | Geen valse fouten meer na annuleren, uitloggen of de veiligheidslimiet. **"Raise the original macOS window before restoring its text field; retain clipboard fallback when the destination cannot be verified."** |
| 4.1.9 | 12 sep | **"Fix a macOS shutdown crash when updating or quitting with Nemotron loaded."** Na een update pas starten als de oude app weg is. |
| 4.1.7 | 12 sep | **"Allow local transcription up to ten minutes instead of cancelling at one minute." "Require observed Windows key-down state before the release watchdog can stop a recording."** Retry-After respecteren. |
| 4.1.6 | 12 sep | Crash bij fractionele hover-afmetingen. **"Reject recording when no transcription backend is available."** Backoff bij een volle schijf of netwerkfouten. |
| 4.1.4 | 11 sep | Anonieme analytics met een schakelaar, en uit als de instellingen onleesbaar zijn. |
| 4.1.2 | 10 sep | Woordenboek-rewrite: scopes per app en taal, correcties vanuit de history, import/export, sync. Experimentele Nemotron-streaming. Recognition hints standaard uit. |
| 4.0.1 | 8 sep | 4.0: nieuw dashboard en kleinere pill. Fn hold-to-talk inclusief "quick taps and releases during microphone startup". Cloud standaard MAI-Transcribe 2. **Annuleren behoudt eigenaarschap tot het native werk klaar is. Modelactivatie wacht op idle en behoudt de oude engine als de nieuwe faalt.** Instellingen overschrijven geen andere instellingen meer. |
| 2.8.4 | 4 aug | Gehallucineerde "..." aan het eind wordt verwijderd, voor **alle** engines inclusief cloud. Update vanaf een DMG geeft nu een blijvende melding met de oplossing: "move to Applications". |
| 2.5.3 | 12 jun | Stille microfoonstream: eerst opnieuw opbouwen, dan uitwijken naar een ander apparaat. **Wait-free ringbuffer.** Loopback-apparaten uitgesloten. Verlopen sessies stoppen met retry'en. **"Push-to-talk requires a real key-up during the first 150ms of a hold."** Doel opnieuw bepalen als het Windows-venster weg is. Frontmost-app in-process in plaats van via AppleScript. Panics op Linux gevangen. Retry bij antivirus-stat-races. Uitgevallen macOS-tap herstelt zichzelf. |
| 2.5.2 | 12 jun | Windows: een idle-watchdog bouwt een dode microfoon opnieuw op, met eenmalige failover. Echte privacyblokkade apart gemeld van "geen microfoon". Quit in het dashboard en uitleg over de tray. |
| 2.5.1 | 11 jun | Polish/Enhance op credits. Parakeet V3. Start- en stopgeluiden. **"Releasing the push-to-talk key now stops recording instantly instead of hanging until a watchdog timer."** Dubbele tray-iconen opgelost. Push-to-talk werkte niet meer na een update op Windows 11. |
| 2.2.57 | 10 jun | Grace-periode voor de opnamepoort tijdens hiccups in de token-refresh. Windows-focus: een neutrale Alt-tik tegen de foreground-lock (AutoHotkey-truc). **"macOS: hotkey dispatch moved off the event-tap callback onto a worker thread."** Model-probe hasht zonder lock. Waveform-re-renders geïsoleerd. Fonts zelf gehost. CI-guard tegen een consuming keyboard hook. Atomische release-tags. |
| 2.2.56 | 9 jun | **Windows: Ctrl+Win stopte na ~25 ms, omdat de consuming hook de key-up voor de watchdog verborg.** Nu een listen-only hook plus een maskeertoets. Geluiden gesynthetiseerd in Rust. Widget-resize vanuit Rust. **Injectie wacht op het vastleggen van de foreground van dezelfde take.** Settings-mutaties als geserialiseerde transacties. Rebind tijdens een opname beëindigt die netjes. |
| 2.2.55 | 9 jun | "Use Recommended"-sneltoets per apparaat. Setup tot een echte globale press én release bewezen is. Enkele modifier toegestaan. Eigen vensters nooit als doel. Tekst blijft op het klembord bij een mislukt herstel. Plakken vereist een bevestiging van de foreground. |
| 2.2.54 | 8 jun | Fn/Globe als standaard op de Mac (fallback Ctrl+Option). Onboarding zet "Press Globe key to: Do Nothing". |
| 2.2.53 | 8 jun | **macOS: diagnostiek-writes in de event tap lieten de tap uitvallen, waardoor de key-up verloren ging.** Nu een aparte writer-thread. De dubbeltik-vergrendeling reset bij elke stop. Widget verbergt zich automatisch. |
| 2.2.52 | 29 mei | Copy-to-clipboard-modus. Geen valse "Pro required" tijdens een refresh. Linux X11: non-ASCII via het klembordpad. |
| 2.2.51 | 27 mei | What's New-modal op basis van de changelog-JSON. |
| 2.2.50 | 13 mei | Server-gestuurde minimumversie (`X-App-Min-Version`). **Het Windows-klembord wordt niet meer teruggezet, om wachtwoorden en 2FA-codes niet opnieuw bloot te stellen.** Een sync-command op Windows veroorzaakte een stack overflow (`__chkstk`); nu async met `spawn_blocking` en een stack van 4 MiB. Kritieke sectie van de audio-mutex teruggebracht tot een memcpy. |
