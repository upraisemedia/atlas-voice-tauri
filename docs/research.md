# Atlas Voice (Tauri) — onderzoek

> Fase 1: onderzoek. Peildatum **29 september 2026**. Versienummers zijn live nagekeken (crates.io, npm, GitHub-releases, officiële docs), niet uit het geheugen. Waar iets een schatting of ervaringsregel is, staat dat erbij.
>
> Opbouw: eerst een samenvatting met de conclusies, daarna per onderwerp de details. Bronnen staan per hoofdstuk en achteraan.

---

## 0. Samenvatting in tien punten

1. **Er bestaat een sterk open-source voorbeeld: [Handy](https://github.com/cjpais/Handy)** (MIT, ~32k sterren, Tauri 2 + Rust + React, lokale Whisper/Parakeet). De bouwstenen van Handy zijn los te gebruiken en MIT-gelicenseerd: `handy-keys` (globale toetsen incl. Fn en key-release), `transcribe-cpp` (één runtime voor Whisper, Parakeet, Canary en Nemotron via GGUF, met Metal en Vulkan) en `transcribe-rs` (ONNX). Handy heeft géén cloud-transcriptie. Lokaal plus cloud is dus ons eigen onderscheid.
2. **BridgeVoice heeft 25 publieke versies met changelog; die heb ik volledig gelezen.** Hun bugs zijn te herleiden tot zo'n vijftien ontwerpregels (§3). De belangrijkste:
   - de event-tap- of hook-callback doet niets behalve een bericht doorsturen;
   - één centrale state machine met een ID per opname (take-ID);
   - timeouts die meeschalen met de audioduur;
   - een expliciete shutdown die het model vrijgeeft vóór de exit;
   - het doel (app en venster) vastleggen bij key-down, en controleren vóór het plakken.
3. **Tauri 2.12.0** (26-09-2026) is de basis. Die versie repareert een tray-bug op macOS 27 (dat draai jij). Tauri 3 is alpha; de migratie ziet eruit als een refactor van Cargo en config, geen herschrijving.
4. **De officiële `global-shortcut`-plugin kan geen Fn en geen modifier-only toetsen.** Daarom `handy-keys` (CGEventTap / WH_KEYBOARD_LL / evdev), met de officiële plugin als fallback.
5. **Standaardmodel: NVIDIA Parakeet TDT 0.6B v3.** 25 Europese talen inclusief Nederlands, automatische taalherkenning, eigen interpunctie en hoofdletters, CC-BY-4.0, snel op CPU. Het is ook het model van de huidige Swift-app. Hoogste NL-kwaliteit: Whisper large-v3 (FLEURS-nl WER 5,6 tegen 7,5 voor Parakeet), maar traag en gevoelig voor hallucinaties.
6. **Runtime-keuze: `transcribe-cpp` (ggml/GGUF) als ene runtime** vermijdt de problemen van ONNX Runtime:
   - `ort` is nog release-candidate;
   - prebuilt binaries eisen AVX2;
   - er zijn geen officiële Intel-Mac-binaries;
   - CoreML is instabiel voor Parakeet.

   Keerzijde: het project is jong (0.2.4). Een korte spike in bouwfase 1 moet dit bevestigen.
7. **Cloud: top-3 voor NL+EN is ElevenLabs Scribe v2, OpenAI gpt-transcribe en Mistral Voxtral.** Scribe v2 heeft de beste gepubliceerde NL-nauwkeurigheid. OpenAI gpt-transcribe is goedkoop en sterk. Voxtral is het goedkoopst en een EU-bedrijf. Kortlevende tokens voor batch-transcriptie bestaan bij geen van deze drie, dus beheerde cloud vraagt een **proxy** (Laravel is prima). BYOK (eigen API-key van de gebruiker) kan zonder backend.
8. **Kosten beheerde cloud:** $2,70–4,00 STT-kosten per zware gebruiker (30 min/dag) per maand. Een abonnement van €10–15 per maand is concurrerend (Wispr Flow $15, Aqua $10, Superwhisper ~$8,50). Paddle als merchant of record regelt de EU-btw.
9. **Distributie: ~$99 per jaar voor macOS.**
   - Developer ID, notarization via een App Store Connect API key, `tauri-action@v1`.
   - Windows-signing via Azure Artifact Signing kan in de EU **alleen als organisatie**. Als particulier is een OV-cloudcertificaat nodig (vanaf ~$115/jaar).
   - Updater via `tauri-plugin-updater` 2.13 (ondersteunt inmiddels ook deb, rpm en MSI).
10. **De Swift-app heeft goed gedrag om te behouden:**
    - Fn push-to-talk;
    - dubbeltik voor hands-free;
    - Esc om te annuleren;
    - een expliciete modeldownload;
    - klembordherstel;
    - een woordenboek.

    Er zitten ook concrete bugs in om níet mee te nemen: Esc na het loslaten plakt toch, een race bij de start, geen focusherstel, geen timeouts en geen afhandeling van apparaatwissels (§1.9).

**Lokaal actiepunt:** Rust staat wel in `~/.cargo/bin` (cargo/rustc 1.98.1, rustup aanwezig), maar je zsh-profiel laadt `~/.cargo/env` niet. `cargo` wordt daardoor niet gevonden in een nieuwe shell. Oplossing: voeg `. "$HOME/.cargo/env"` toe aan `~/.zshenv` of `~/.zprofile`. Ik heb je profiel niet aangepast.

---

## 1. De bestaande Swift-app (`~/sites/atlas-voice`)

Alle 33 Swift-bestanden (3809 regels) zijn gelezen, plus de README, CLAUDE.md, `docs/plans/*`, Info.plist, de entitlements en de git log.

### 1.1 Architectuur en pipeline
- Menubalk-agent (`LSUIElement`), bundle-ID `nl.atlasvoice.app`, macOS 26+, geen sandbox, geen hardened runtime.
- `AppContainer` is de compositie-root. `DictationCoordinator` is de pipeline:
  **hotkey → `beginDictation` → AVAudioEngine-tap (16 kHz mono f32) → loslaten → `session.finish()` (Parakeet) → refiner-keten → `TextInjector` → history**.
- Eén fase-enum (`idle / listening / transcribing / refining / inserting / error`) stuurt de overlay, het menubalk-icoon en de UI. Dat is een goed patroon om te behouden.

### 1.2 Toets (hotkey)
- `CGEventTap` listen-only op `flagsChanged | keyDown`, main run loop. Start pas als `AXIsProcessTrusted()` waar is; tot die tijd pollt de app elke 2 s. Een tap die uitgezet wordt, zet de app opnieuw aan.
- Bindingen: **Fn (standaard)**, Ctrl+Opt, Opt+Cmd, Rechter-Cmd (keycode 54). De modifierset moet exact overeenkomen.
- **Hands-free:** een tik korter dan 0,35 s, gevolgd door een tweede tik binnen 0,5 s, vergrendelt de opname. De volgende druk stopt. **Esc** (keycode 53) annuleert.
- Geen minimale opnameduur in de coordinator en geen debounce. Maximum 10 min, daarboven worden samples **stil weggegooid**.

### 1.3 Audio
- Eén `AVAudioEngine`. De tap wordt pas **na** de toetsdruk en na `makeSession` geïnstalleerd, dus het begin van een zin kan wegvallen (vooral bij Bluetooth/HFP).
- Microfoon instelbaar via CoreAudio-UID. Een verdwenen apparaat valt stil terug op de systeemstandaard.
- Resampling via `AVAudioConverter` naar 16 kHz. Niveaumeter: RMS van −50 tot 0 dB.
- **Geen VAD en geen stilte-trim. Apparaatwissels worden niet afgehandeld:** de engine stopt en de app blijft in `.listening` hangen.

### 1.4 Transcriptie
- **Parakeet TDT 0.6B v3** via FluidAudio 0.15.7 (CoreML/ANE, int8-encoder). Op schijf 461 MB in `~/Library/Application Support/Atlas Voice/Models/`.
- Download alleen expliciet, met voortgang, stop, opnieuw en verwijderen. Geen eigen checksum.
- FluidAudio knipt in stukken van 15 s en eist minimaal 0,3 s audio. Geen timeouts.
- De taalinstelling is voor Parakeet slechts een "hint" en doet voor Latijnse schriften praktisch niets.
- **Vocabulary boosting** (CTC-model plus spelling) is geprobeerd en teruggedraaid. Het werkte slecht op de echte stem ("Claude Code" werd "Coke").

### 1.5 Nabewerking (refinement)
Een keten in deze volgorde:
1. **RuleBased**: vulwoorden eruit (uh, uhm, ehm …), witruimte opschonen, hoofdletter aan het begin, punt aan het eind bij 3+ woorden.
2. **FoundationModels**: Apple Intelligence met een Nederlandse opschoon-prompt. Werkte in de praktijk niet ("modelNotReady").
3. **Dictionary**: regex met Unicode-woordgrenzen, langste match eerst.

Twee zwakke punten: de schakelaar `refinementEnabled` zet ook het woordenboek uit, en de LLM heeft geen timeout of controle op de uitvoer.

### 1.6 Plakken (injectie)
1. Snapshot van het klembord (alle items, alle types).
2. `setString`.
3. Cmd+V (keycode 9) via `CGEvent` op `.cghidEventTap`.
4. Na **300 ms** het klembord terugzetten.

Wat ontbreekt:
- focusherstel;
- een AX-fallback;
- markers voor klembordmanagers (Transient/Concealed);
- layout-bewuste V (Dvorak/AZERTY).

### 1.7 Overlay
- `NSPanel` borderless en non-activating, level `.statusBar`, zichtbaar op alle Spaces en boven full-screen. Klikdoorlaat behalve op de pil zelf.
- Pil van 30 pt: in `listening` een waveform met 9 balkjes; in `transcribing` een pulserende stip; bij een fout een oranje ⚠︎ met tekst.
- Positie: onderaan het scherm met de muis, 16 pt boven de Dock.
- De design-tokens (`#0B0F16` navy, mint `#7FE3E0`, teal `#2FAFB0`) zijn bruikbaar voor de React-UI.

### 1.8 Instellingen, opslag en UI
- **Instellingen:** hotkey, taal, refinement, overlay, geluiden (Tink/Pop), hands-free, history, uiterlijk, microfoon.
- **History:** `history.json`, max 500 records en 30 dagen, **geen audio**. Statistieken: woorden, WPM, "× sneller dan typen", streaks.
- **Woordenboek** (`dictionary.json`) en een kladblok. Snippets en Stijl zijn nog placeholders.
- **Hoofdvenster:** sidebar met Dictatie, Kladblok, Woordenboek, Snippets, Stijl en Instellingen. Opent automatisch als een permissie of het model ontbreekt. Starten bij inloggen via `SMAppService`.

### 1.9 Bugs en zwakke plekken (niet meenemen)

| # | Probleem | Les voor de nieuwe app |
|---|---|---|
| 1 | Esc tijdens transcriberen of refinen plakt toch (geen cancel-check) | Take-ID en cancel-token; verouderde resultaten weggooien |
| 2 | Race: `.listening` gezet vóór de sessie bestaat. Wie loslaat tijdens de init laat de opname hangen | "Release pending" onthouden tijdens arming |
| 3 | Opnames van 0,1–0,3 s geven een foutmelding in plaats van niets te doen | Minimale duur plus energie/VAD: stil negeren |
| 4 | Een losse tik op Fn (hands-free aan) kan onzin plakken | VAD op lege opnames |
| 5 | Esc en Fn gaan door naar de voorgrond-app | Esc alleen tijdens een actieve opname onderscheppen |
| 6 | Wissel van microfoon of Bluetooth-HFP wordt niet afgehandeld; "Systeemstandaard" terugzetten werkt niet | Stream opnieuw opbouwen, apparaat opnieuw kiezen |
| 7 | Doel-app op twee verschillende momenten bepaald | Doel eenmalig vastleggen bij key-down |
| 8 | Fouttimer zonder token, en een toegankelijkheidsmelding die permanent blijft staan | Meldingen met ID's |
| 9 | Limiet van 10 min kapt stil af | Waarschuwen en netjes afronden |
| 10 | Geen timeouts (laden, transcriptie, LLM) | Timeouts die meeschalen met de audioduur |
| 11 | Klembord herstel na vaste 300 ms, zonder markers | Markers plus eigendomscheck |
| 12 | "Tink"-geluid kan in de opname komen | Geluid spelen ná de eerste samples, of buiten de opname |

**Wat ontbrak en op de roadmap stond:**
- onboarding met microfoontest;
- Fn volledig claimen;
- AX-injectie;
- snippets;
- stijl per app;
- command mode;
- cloud-refiner;
- Developer ID met notarization en updates;
- VAD;
- model ontladen bij inactiviteit;
- streaming.

---

## 2. Referentieproduct: BridgeVoice (BridgeMind)

### 2.1 Product
- **Platforms:** macOS 14+ (universal), Windows 10/11 x64, Linux AppImage (X11 en Wayland via portals).
- **Bediening:**
  - push-to-talk: Fn/Globe op de Mac (fallback Ctrl+Option), Ctrl+Win op Windows, Ctrl+Alt op Linux; ook een enkele modifier;
  - toggle;
  - click-to-record;
  - dubbeltik voor hands-free.
- **Lokale modellen:**

  | Model | Grootte | Opmerking |
  |---|---|---|
  | Parakeet V3 | 671 MB | |
  | Whisper Turbo Q5 | 574 MB | |
  | Distil-Whisper v3.5 | 1,52 GB | alleen EN |
  | Whisper Large v3 | 3,10 GB | |
  | Parakeet Unified EN | 663 MB | niet op Intel |
  | Nemotron streaming | — | experimenteel |

- **Cloud:** Microsoft MAI-Transcribe-2 via OpenRouter, gerelayd door hun eigen API.
- **Prijs:** onderdeel van BridgeMind Pro, $50/maand (tijdelijk $25). **Inloggen en Pro verplicht, ook voor lokaal gebruik.**
- **Extra's:**
  - Polish/Enhance: AI-herschrijven op credits;
  - woordenboek met scopes per app en taal, import/export en sync;
  - statistieken;
  - start- en stopgeluiden;
  - een widget die zichzelf verbergt;
  - een "kopieer naar klembord"-modus;
  - anonieme analytics die je uit kunt zetten;
  - Sentry.

### 2.2 Changelog (v2.2.50 t/m v4.1.18, mei–sep 2026)
Het volledige overzicht per versie staat in [`research-bridgevoice-changelog.md`](research-bridgevoice-changelog.md). Hieronder de lessen per categorie.

## 3. Ontwerpregels afgeleid uit BridgeVoice, Handy en de Swift-app

Dit is de kern van het onderzoek: problemen die anderen pas ná hun release tegenkwamen.

### 3.1 Toets en loslaten
- **Windows:** een consuming low-level hook slikte de eigen key-up in, waardoor de opname na ~25 ms stopte (BV 2.2.56). Pollen gaf fantoom-releases binnen 100 ms (2.5.3, 4.1.7).
  → **De hook is altijd listen-only.** Het Startmenu onderdruk je met een neutrale maskeertoets (zoals AutoHotkey dat doet).
- **Een release telt alleen na een waargenomen key-down.** Minimale vasthoudtijd en een release-grace van ~50 ms (tegen X11-autorepeat, Handy #1539). Een watchdog die `GetAsyncKeyState` pollt is een vangnet, nooit de bron.
- **macOS:** I/O of emits in de tap-callback lieten de tap time-outen; macOS schakelde hem uit en de key-up ging verloren (BV 2.2.53, 2.2.57).
  → **De callback doet alleen een lock-free send naar een kanaal.** Heractiveer de tap *in* de callback bij `TapDisabledByTimeout`. Pollen met `CGEventTapIsEnabled` in een loop lekte IPC-vouchers en veroorzaakte een kernel panic (Handy #1827).
- **Secure Input** (wachtwoordvelden, Terminal "Secure Keyboard Entry") blokkeert keyDown voor event taps. Handy detecteert `IsSecureEventInputEnabled()` en registreert dan tijdelijk via Carbon (Handy #1578).
- **Windows-hooks verdwijnen** na slaap, Win+L en RDP, en na een trage callback (>1 s timeout, zonder melding). → Opnieuw installeren bij `WM_WTSSESSION_CHANGE` en `WM_POWERBROADCAST`.
- Wie tijdens een opname de sneltoets wijzigt of pauzeert, beëindigt die opname netjes.
- **Onboarding verifieert een echte globale press én release** voordat opnemen mag. Telemetrie op "instant stops" van minder dan 100 ms.

### 3.2 Levenscyclus van een opname
- **Eén state machine in Rust:** Idle → Arming → Recording → Transcribing → (Postprocess) → Injecting → Idle, met een **take-ID** per opname. Resultaten van een oude take worden weggegooid.
- **Annuleren blijft eigenaar tot het native werk klaar is.** Opruimen vóór een nieuwe take mag starten (BV 4.0.1, 4.1.10).
- "Geannuleerd", "leeg" en "limiet bereikt" zijn **uitkomsten, geen fouten**.
- **Modelwissel is een transactie:** wacht op Idle, laad het nieuwe model, wissel pas bij succes.
- **Geen opname starten als er geen backend beschikbaar is** (BV 4.1.6). Dat is beter dan achteraf falen.
- Handy's modi zijn een goed voorbeeld: PushToTalk / Toggle / HoldOrToggle (kort tikken vergrendelt, lang vasthouden = push-to-talk), 30 ms debounce, en een druk tijdens de verwerking onthouden of vergeten.

### 3.3 Audio
- **De audio-callback is wait-free:** een SPSC-ringbuffer (`rtrb`), vooraf aangeraakt tegen page faults. Geen mutex, allocatie of logging (BV 2.5.3; Handy #1930 bracht `coreaudiod` in de war).
- **Start de microfoon zo vroeg mogelijk.** Opzoeken van het doelvenster en voorbereiden van het woordenboek mogen de start niet vertragen (BV 4.1.12).
- **Readiness pas na de eerste echte samples:** overlay-kleur en startgeluid (Handy #1283 en #1879: eerste woorden kwijt).
- **Stille of dode stream:** eerst opnieuw opbouwen op hetzelfde apparaat, dan uitwijken naar een ander, dan pas een banner (BV 2.5.2, 2.5.3). Sluit loopback-apparaten (Stereo Mix, virtuele kabels) uit van automatische keuze.
- **Bluetooth:** AirPods schakelen naar HFP (16/24 kHz) zodra de microfoon opengaat. cpal meldt dan `StreamInvalidated`, en de app moet de stream opnieuw opbouwen. Handy sluit de microfoon **lui** (na 30 s inactiviteit) zodat een headset niet steeds heen en weer schakelt.
- **VAD/trim met korte vensters en ruime padding.** Een harde volumedrempel verloor zachte woorden (BV 4.1.13). Negeer bijna-stille opnames en losse klikken wel.

### 3.4 Focus en doel
- **Leg het doel vast bij key-down, asynchroon:** app, venster en bij voorkeur het gefocuste element. Sluit eigen vensters uit (BV 2.2.55).
- **Herstel in stappen:** venster naar voren, element focussen, controleren dat het nu vooraan staat, dan plakken. Lukt het niet, laat de tekst dan op het klembord staan en toon een duidelijke melding (BV 4.1.10, 4.1.17, 2.2.55).
- **Windows:** de foreground lock valt te omzeilen met een neutrale Alt-tik (AutoHotkey-truc, BV 2.2.57). Is het doelvenster verdwenen, bepaal het opnieuw.
- **macOS:** zoek de voorgrond-app op in het proces zelf (NSWorkspace), niet via AppleScript (BV 2.5.3).
- De injectie wacht op het vastleggen van het doel van *dezelfde* take. Een snelle tik plakte anders in het vorige doel (BV 2.2.56).

### 3.5 Plakken en klembord
- **Klembord herstellen op een vaste timer is het grootste open probleem bij Handy** (#502, 89 comments). Klembordmanagers zoals Alfred lezen het klembord binnen ~100 ms, en trage Electron-apps plakken dan de oude inhoud.
  → Markeer met **`org.nspasteboard.ConcealedType`/`TransientType`** (macOS) of `ExcludeClipboardContentFromMonitorProcessing` (Windows). Zet alleen terug als `changeCount` nog van ons is, en maak de vertraging instelbaar.
- **BridgeVoice zet het klembord op Windows bewust níet meer terug.** Anders kwamen wachtwoorden en 2FA-codes opnieuw bloot te staan (2.2.50). Klembordherstel wordt daarmee een bewuste keuze in de instellingen.
- **Snapshot van álle pasteboard-types**, niet alleen tekst. `arboard` kan dat niet, dus zelf doen via `NSPasteboard.pasteboardItems`.
- **Layout-bewuste Cmd+V:** met `UCKeyTranslate` de toets zoeken die "v" geeft (Handy #93, #439).
- **macOS:** `CGEventSource(.privateState)`, zodat ingedrukte hotkey-modifiers niet meelekken (idee uit VoiceInk).
- **Windows:** eerst fysiek ingedrukte modifiers loslaten, daarna terugzetten. Terminal gedetecteerd → Ctrl+Shift+V (OpenWhispr, MIT).
- **Unicode altijd via het klembord.** Nooit als ASCII-toetsaanslagen typen (BV 2.2.52, Linux).
- **Eén normalisatiestap voor élke engine, ook cloud:** trailing "..." weg, witruimte opschonen, woordenboek toepassen (BV 2.8.4).
- `enigo` 0.6.1 crasht op macOS buiten de main thread (fix #510 niet uitgebracht). → Eigen `CGEventPost` op de main thread.

### 3.6 Trage modellen en timeouts
- **BridgeVoice brak lokale transcriptie na 60 s af** en ving zo trage modellen. De grens ging naar 10 min (4.1.7).
  → Maak de timeout afhankelijk van de audioduur en de modelsnelheid, met een heartbeat en een altijd werkende annuleerknop.
- Een **maximale opnameduur** met een waarschuwing vooraf, die netjes afgehandeld wordt als "limiet bereikt" en niet als fout.
- **Lange opnames** in stukken verwerken (Whisper heeft een venster van 30 s; Parakeet kan tot 24 min in één keer, maar de geheugenpiek is groot). Handy verloor lange opnames stil (#783, #1332).

### 3.7 Afsluiten en crashes
- **Crash bij afsluiten of updaten met Nemotron geladen:** GPU-resources werden niet vrijgegeven (BV 4.1.9). Handy zet `GGML_METAL_NO_RESIDENCY=1` tegen een crash bij het opruimen van Metal (#1902).
- **Tauri-valkuilen:**
  - `ExitRequested` vuurt **niet** bij Cmd+Q of Dock→Quit (#9198);
  - managed `State` wordt bij exit **niet gedropt** (#14420);
  - `cleanup_before_exit` buiten de main thread crasht (#12534).

  → **Een eigen Quit-pad:** opname stoppen, inferentie-thread joinen, model expliciet `take()`/drop, en pas dan `app.exit(0)`. `RunEvent::Exit` als vangnet.
- **De updater wacht tot het oude proces weg is** (single-instance). Bij een draaiende app vanaf een DMG of vanuit `/Volumes`: vraag om naar Programma's te verplaatsen (BV 2.8.4).
- **Zwaar werk nooit in een synchroon command.** Een sync command draait op de main thread; op Windows gaf dat een stack overflow (`__chkstk`, BV 2.2.50).
- `catch_unwind` op de grens van commands en worker-threads. Tijdens inferentie haalt Handy de engine *uit* de mutex; bij een panic laadt hij het model opnieuw, zodat de mutex nooit vergiftigd raakt.
- `dragDropEnabled: false` als je geen file-drop gebruikt (BV 4.1.14). UI-afmetingen afronden voordat ze naar native gaan (BV 4.1.6).

### 3.8 Modeldownload
- Download naar een `.part`-bestand, met Range-resume, een stall-watchdog (60 s), een verplichte **sha256** en een atomische rename. Gebruik `Retry-After` en probeer 4xx (behalve 408/429) niet opnieuw (BV 4.1.7). Op Windows korte retries bij rename tegen virusscanners (BV 2.5.3).
- Valideer met een hash of manifest, niet met mtime. Hash op de blocking pool, zonder de lock van de modelmanager vast te houden (BV 2.2.57, 4.1.6).
- Ontbreekt het gekozen model, val dan terug op het beste gedownloade model en meld dat (BV 2.5.3).
- Voortgang maximaal ~10 keer per seconde naar de UI.

### 3.9 UI en overlay
- **Grootte en positie van het overlay vanuit Rust**, begrensd tot het werkgebied van de monitor (BV 2.2.56). Alle geometrie-aanroepen op de main thread (Handy #227).
- **Een fout maakt een verborgen widget zichtbaar.** Toasts: één regel, met details bij hover.
- **Audioniveaus:** `emit_to("overlay")` met ~30 fps throttle. Niets sturen naar een verborgen overlay, anders groeit WebKit-geheugen onbeperkt (Handy #1279).
- **Aparte Vite-entry per webview** (overlay vs. hoofdvenster). Fonts zelf hosten. Waveform geïsoleerd en gememoized (BV 2.2.57).
- **`transparent: true` op macOS kost ~8× GPU-vermogen, zelfs bij stilstand** (Tauri #15471). → Overlay met `hide()` verbergen als er niet gedicteerd wordt.
- Eén tray-instantie. Expliciete Quit, plus een eenmalige uitleg dat de app in de tray blijft (BV 2.5.1, 2.5.2).

### 3.10 Instellingen, auth en diagnostiek
- **Instellingen via één "actor"** met geserialiseerde read-modify-write-transacties en atomische writes. Bij een parse-fout: "salvage" (geldige velden behouden). Gelijktijdige writes draaiden elkaar terug (BV 2.2.56).
- **Auth (voor beheerde cloud):**
  - fouten classificeren (terminaal vs. tijdelijk);
  - opnemen mag nooit afhangen van een live token-check, gebruik een gecachte entitlement met grace-periode;
  - **lokale transcriptie zonder enige netwerk-check**.

  Een groot deel van de BridgeVoice-changelog gaat over auth-bugs.
- **Privacy-veilige diagnostiek vanaf dag één:** timings per fase, stopoorzaak van de toets en fouttype. Nooit transcripttekst, audio of apparaatnamen. Opt-out die ook geldt als de instellingen niet te lezen zijn.

### 3.11 Linux
- AppImage: **bundel geen `libwayland-client`/`libvulkan`** (lege of witte vensters met recente Mesa, BV 4.1.18). Bouw op Ubuntu 22.04 voor glibc-compatibiliteit.
- Wayland heeft geen globale hooks en geen vrije injectie. Opties:
  - portals: GlobalShortcuts met Activated/Deactivated, en RemoteDesktop;
  - evdev (groep `input`);
  - wtype/ydotool.
- Geen SIGUSR1 gebruiken (WebKitGTK-GC, Handy #1793).

---

## 4. Tauri 2

### 4.1 Versies (29-09-2026)
| Pakket | Versie |
|---|---|
| `tauri` crate | **2.12.0** (26-09-2026) |
| `@tauri-apps/cli`, `@tauri-apps/api` | 2.12.0 |
| `create-tauri-app` | 4.7.4 |
| `tauri-action` (CI) | **v1** (breaking t.o.v. v0) |
| Tauri 3 | 3.0.0-alpha.3 |

- **2.12** brengt:
  - fix voor tray-klikken op macOS 27 (#16088);
  - Xcode 27;
  - `exit()` in de core-API;
  - per-webview gescopede Channels (security);
  - einde van Windows 7-ondersteuning;
  - MSRV 1.90.
- **Tauri 3 (alpha):**
  - runtime als aparte crate (wry of **CEF/Chromium**);
  - `macos-private-api` verhuist naar de runtime;
  - GTK4;
  - Stronghold verdwijnt.

  → Houd `macOSPrivateApi` en runtime-specifieke calls in één module.
- `~/Desktop/tauri-test` gebruikt tauri 2.11.5 met een **Vue**-template, dus geen directe code-referentie voor React.

### 4.2 Plugins
| Plugin | Versie | Gebruik |
|---|---|---|
| global-shortcut | 2.4.0 | Press en Release. **Geen Fn en geen modifier-only**, alleen X11. Fallback. |
| tray (core feature `tray-icon`) | — | Menubalk en tray |
| notification | 2.5.0 | Meldingen |
| updater | 2.13.0 | Ondertekende updates, ook deb/rpm/MSI |
| autostart | 2.6.0 | Starten bij inloggen (LaunchAgent) |
| store | 2.5.0 | JSON key-value. Geen secrets. |
| stronghold | 2.4.0 | **Deprecated**, verdwijnt in v3. Niet gebruiken. |
| single-instance | 2.5.0 | Als eerste plugin registreren |
| window-state | 2.5.0 | Alleen hoofdvenster (overlay uitsluiten) |
| log | 2.10.0 | Bestandsrotatie op grootte |
| process | 2.4.0 | `relaunch()` na update |
| clipboard-manager | 2.4.0 | Plakt niet in andere apps. Beperkt nut. |
| deep-link | 2.5.0 | `atlasvoice://` (login-callback) |
| dialog, fs, http, os, opener, positioner | 2.4–2.8 | Naar behoefte. `http`/`fs` niet nodig als Rust dit doet. |

**Secrets:** er is geen officiële keyring-plugin. Gebruik `keyring-core` 1.0 met de native stores (of `keyring` 4.2) direct in Rust.

### 4.3 Capabilities en permissies
- Capability-bestanden koppelen permissies aan **venster-labels**. Staat een venster in meerdere capabilities, dan worden de permissies samengevoegd (union). Gebruik dus geen `"windows": ["*"]`.
- **Valkuil:** zonder app-manifest in `build.rs` (`AppManifest::new().commands(&[...])`) zijn **alle eigen commands voor elk venster** beschikbaar. Met een manifest krijgt het overlay alleen `listen` plus een paar leescommands.
- Houd keys, bestandssysteem en netwerk in Rust, niet in JS-plugins. Gebruik een strikte CSP.

### 4.4 IPC
- **Commands:** async plus `spawn_blocking` of een eigen thread voor zwaar werk. Een sync command draait op de main thread.
- **Events** (JSON, niet voor hoge frequentie): statuswijzigingen, `emit_to(label)`.
- **Channels:** geordend en snel, per aanroep. Voor downloadvoortgang en eventueel audioniveaus.
- **State:** `app.manage()` met een std `Mutex`. Zware subsystemen (audio, inferentie, toetsen) krijgen een eigen thread met een berichtenkanaal. Commands sturen alleen berichten.
- **`tauri-specta`** (2.0.0-rc.21) genereert TypeScript-types voor commands en events. Nog een RC, maar Handy draait er 116 commands op.

### 4.5 Overlay-venster
- **macOS: `tauri-nspanel` 2.1.0** (sinds 19-09-2026 op crates.io). NSPanel met `nonactivating_panel`, level `Status`, `can_join_all_spaces` + `full_screen_auxiliary` en `can_become_key_window: false`. Gewone `focusable(false)` op een NSWindow steelt nog steeds focus bij een klik (#14102).
- **Windows:** `focusable(false)` (= `WS_EX_NOACTIVATE`), plus bij elke show `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`. Bekend: zwarte transparantie op Win10 (#15947). Gebruik `noRedirectionBitmap` (2.12) tegen een witte flits.
- **Linux:** X11 werkt. Wayland staat geen positie of always-on-top toe. `gtk-layer-shell` werkt wel op wlroots en KDE, niet op GNOME.
- **Geen Dock-icoon:** `ActivationPolicy::Accessory` vóór `run()` zetten. Tijdelijk `Regular` terwijl het hoofdvenster open is, zoals de Swift-app doet.

### 4.6 Webviews
| Platform | Webview | Opmerkingen |
|---|---|---|
| macOS | WKWebView | Safari-versie van het OS |
| Windows | WebView2 | Standaard aanwezig op Win10+/11. Installatiemodus `downloadBootstrapper`. |
| Linux | WebKitGTK 4.1 | Trager. Problemen met NVIDIA/DMABUF (`WEBKIT_DISABLE_DMABUF_RENDERER=1`). |

Omdat Rust de audio doet is er geen `getUserMedia` nodig, en dus geen mediapermissie-prompts in de webview. Houd de overlay-UI licht.

### 4.7 macOS-bundling
- `Info.plist` met `NSMicrophoneUsageDescription`.
- `Entitlements.plist` met `com.apple.security.device.audio-input`. Hardened runtime staat standaard aan.
- **Accessibility en Input Monitoring zijn TCC-rechten**, geen entitlement.
- `macOSPrivateApi: true` (voor transparantie en NSPanel) betekent: **niet in de Mac App Store**. Buiten de App Store geen probleem.

---

## 5. Rust-crates

| Doel | Crate | Versie | Notities |
|---|---|---|---|
| Audio-capture | `cpal` | 0.18.2 | Grote breaking release. `play()` expliciet, fouttypes `DeviceChanged`/`StreamInvalidated`. PipeWire/Pulse op Linux als opt-in feature. 0.19 brengt weer een API-breuk. |
| Ringbuffer | `rtrb` | 0.4 | Lock-free SPSC (Handy) |
| Resampling | `rubato` | 5.0 | Realtime-veilig. **API wisselt vaak** (0.16 → 5 in een jaar): pin de major-versie. |
| VAD | `earshot` | 1.2.2 | Pure Rust, geen ONNX, ~95 KiB. Silero via ORT is het alternatief. |
| ASR, één runtime | `transcribe-cpp` | 0.2.4 | ggml/GGUF. Whisper, **Parakeet v3**, Canary, Nemotron 3.5, Moonshine … Metal, Vulkan, CUDA. Door Handy en Epicenter gebruikt. Jong. |
| ASR, ONNX | `transcribe-rs` / `parakeet-rs` | 0.3.11 / 0.3.8 | Via `ort` 2.0.0-rc.13 (nog RC). Prebuilds vereisen AVX2. Geen Intel-Mac-binary. CoreML instabiel voor Parakeet. |
| ASR, klassiek | `whisper-rs` | 0.16.0 | whisper.cpp 1.8.3 (upstream 1.9.4). ~6 maanden stil. |
| Toetsen | `handy-keys` | 0.3.4 | Fn, modifier-only, press/release, blokkeren, herstel van tap/hook, UI-recorder. MIT. |
| Toetsen, fallback | `global-hotkey` / plugin | 0.8.0 / 2.4.0 | Combinaties als Ctrl+Alt+Space |
| Wayland-portals | `ashpd` | 0.13.13 | GlobalShortcuts (Activated/Deactivated) |
| macOS-FFI | `objc2`, `objc2-core-graphics`, `-app-kit`, `-application-services`, `-io-kit` | 0.6.4 / 0.3.2 | Permissies, CGEventPost, NSPasteboard, AX |
| Klembord | `arboard` | 3.6.1 | Heeft `exclude_from_history()`, maar **geen backup van alle types**. Crash bij gelijktijdige mutatie (#218). → Op macOS zelf via objc2. |
| Toetssimulatie | `enigo` | 0.6.1 | **Crasht buiten de main thread op macOS.** → Eigen CGEventPost/SendInput. |
| Secrets | `keyring-core` + native stores | 1.0 | Keychain / Credential Manager / Secret Service |
| Logging | `tracing` + `tauri-plugin-log` | 0.1.44 / 2.10 | Nooit tekst of audio loggen |
| Crashes | `sentry` + `tauri-plugin-sentry` | 0.49.3 / 0.7 | Opt-in, PII uit, minidumps kunnen gevoelig zijn |
| Downloads | `reqwest` + `sha2` | 0.13.5 / 0.11 | Range-resume, streaming hash |
| Geschiedenis | `rusqlite` + `rusqlite_migration` | 0.37 | Zoals Handy |
| Wegwerp | `rdev`, `device_query`, `sherpa-rs`, `voice_activity_detector`, `inputbot` | — | Dood, gearchiveerd of met conflicterende versie-pins |

**Threads:** inferentie is blokkerend CPU/GPU-werk en draait op een eigen `std::thread` met een kanaal, nooit op de tokio-executor. Audio-callbacks en toets-hooks hebben hun eigen OS-threads. Tokio alleen voor I/O (downloads, cloud, IPC).

---

## 6. Modellen

| Model | NL | WER EN | WER NL (FLEURS / MLS) | Download | Licentie | Opmerking |
|---|---|---|---|---|---|---|
| **Parakeet TDT 0.6B v3** | ✅ auto-detect | 6,34 (Open ASR) | 7,48 / 12,78 | GGUF Q5_K_M 549 MB, Q8_0 740 MB · ONNX int8 ~670 MB | CC-BY-4.0 | Eigen interpunctie en timestamps, geen Whisper-hallucinaties. Taal niet te forceren. Het model van de huidige Swift-app. |
| **Whisper large-v3** | ✅ | 7,44 | **5,57** / 12,08 | GGML q5_0 1081 MB, f16 3095 MB | MIT | Beste NL, maar traag (Handy-speedscore 23). Hallucineert bij stilte → VAD en een vaste taal. |
| **Whisper large-v3-turbo** | ✅ | 7,83 | (geen NL-cijfer; "minor degradation") | q5_0 **574 MB**, q8_0 874 MB | MIT | Goede balans op Apple Silicon |
| **Canary-1B-v2** | ✅ (taal verplicht) | — | 6,12 / 11,27 | GGUF Q5_K_M 837 MB · ONNX int8 ~1,03 GB | CC-BY-4.0 | Alternatief voor hoge kwaliteit zonder hallucinaties |
| Parakeet TDT 0.6B v2 | ❌ EN | **6,05** | — | ~650 MB | CC-BY-4.0 | Beste voor alleen Engels |
| distil-large-v3.5 | ❌ EN | 7,08 (OOD) | — | ~1,5 GB | MIT | Niet zinvol voor NL-publiek |
| Nemotron 3.5 ASR Streaming 0.6B | ✅ "transcription-ready" | 7,91 | 11,46 (chunk 1,12 s) – 14,03 (80 ms) | GGUF Q4_K_M 496 MB | OpenMDW-1.1 | **Streaming** (live tekst). Kandidaat voor v2. |
| Whisper small / base | ✅ | — | 16,4 / 33,0 | 190 / 60 MB | MIT | Alleen voor zeer zwakke hardware |
| Cohere Transcribe 03-2026 | ✅ (14 talen) | **5,42** | n.b. | ≥1,5 GB | Apache-2.0 | Geen timestamps, hallucineert bij stilte. Later bekijken. |
| Voxtral Mini 4B Realtime | ✅ | — | 7,07 | 2,8 GB | Apache-2.0 | Te zwaar lokaal |
| Moonshine, Kyutai, SenseVoice, Granite | ❌ | | | | | Geen Nederlands |

**Bronnen:** HF-modelkaarten, Canary/Parakeet-paper (arXiv 2509.14128, tabel 11), Whisper-paper, Open ASR Leaderboard PR #208, Handy's `catalog.json`.

**Hosting:**
- Begin met Hugging Face, gepind op een commit (`/resolve/<sha>/<file>`) met sha256-controle. Handy's `handy-computer/*-gguf`-repo's zijn gepind en gehasht.
- Later een eigen mirror op R2/CDN, met HF als fallback. Handy doet dat met `blob.handy.computer`.
- Licenties: een scherm "Modellicenties" en een LICENSE/NOTICE per modelmap. CC-BY vraagt naamsvermelding en het melden van de conversie of quantisatie.

---

## 7. Cloud-transcriptie

### 7.1 Aanbieders
| Aanbieder / model | Prijs (batch) | NL | Keyterms | EU / privacy | Kortlevend token voor batch |
|---|---|---|---|---|---|
| **ElevenLabs Scribe v2** | $0,22/u (+$0,05 keyterms) | **≤5% WER ("Excellent")** | 1000 termen | EU-residency en zero-retention alleen Enterprise | Alleen realtime |
| **OpenAI gpt-transcribe** | $0,0045/min | Ja | prompt + keywords | Geen training. EU-residency na goedkeuring. | Alleen realtime (client secrets) |
| **Mistral Voxtral Mini Transcribe V2** | **$0,003/min** | Ja (FLEURS ~4,9%) | 100 termen | **Frans bedrijf, EU-endpoint.** 30 dagen retentie tenzij ZDR. | Nee → proxy |
| Deepgram Nova-3 multi | $0,0043–0,0052/min | Ja | +$0,0013/min | EU-endpoint. Listprijs veronderstelt deelname aan "Model Improvement"; opt-out ~2× duurder. | **Ja (JWT)** |
| AssemblyAI Universal-3.5 Pro | $0,21/u | Ja | Ja | **EU-regio voor dezelfde prijs** | Alleen streaming |
| Groq whisper-large-v3-turbo | $0,04/u, **minimaal 10 s per request gefactureerd** | Whisper-niveau | prompt | US | Nee |
| Speechmatics | $0,24–0,40/u | Ja | Ja | UK/EU, batch na 7 dagen gewist | **Ja** |
| Google Chirp 3 / Azure | $0,016/min / ~$1/u | Ja | Ja | EU | Complex / STS-token |

**Top-3 voor NL+EN:** Scribe v2 (kwaliteit), gpt-transcribe (prijs en kwaliteit), Voxtral (prijs en EU). Latency voor korte clips wordt waarschijnlijk gedomineerd door netwerk en wachtrij (schatting 0,3–1,5 s). **Zelf benchmarken met Nederlandse clips.**

**Audioformaat:** 16 kHz mono WAV kost ~1,9 MB/min. Prima voor v1. FLAC halveert dat; Opus is ~180 KB/min maar vraagt libopus (C-dependency).

### 7.2 Twee modellen
- **(a) BYOK:** de gebruiker plakt een eigen API-key (opgeslagen in de Keychain) en de app praat direct met de aanbieder. **Geen backend**, geen kosten voor ons. Privacy: de gebruiker is zelf klant van de aanbieder.
- **(b) Beheerde cloud met account:**
  - Kortlevende tokens voor batch bestaan alleen bij Deepgram, Speechmatics en Azure, niet bij de top-3. → **Proxy.**
  - Audio is klein. Een extra hop binnen de EU kost ~20–80 ms. De key blijft geheim, de metering is exact en je kunt van aanbieder wisselen zonder client-update.

### 7.3 Minimaal Laravel-backend (voor b)
- **Auth:** de app opent de browser met PKCE, de gebruiker logt in (magic link of Socialite), de site stuurt terug naar `atlasvoice://auth?code=…` (deep-link) en de app wisselt de code in voor een **Sanctum**-token (ability `transcribe`) dat in de Keychain gaat. Alternatief: Passport 13 device-flow.
- **`POST /api/v1/transcriptions`:** multipart met audio, taal, keyterms en `request_id` (idempotent). Middleware `auth:sanctum` + `throttle` + saldocheck. Guzzle stuurt de audio door naar de aanbieder. Boeken gebeurt in één transactie met `lockForUpdate`.
- **Tabellen:** `usage_events` (audio_ms, billed_ms, cost, latency, status) en een append-only `credit_ledger`.
- **Misbruik voorkomen:**
  - e-mailverificatie;
  - kleine trial (bijv. 30 min);
  - tokens per device, intrekbaar;
  - concurrency-slot per gebruiker (`Redis::funnel`);
  - maximaal 10 min en 25 MB per clip;
  - fair-use-plafonds;
  - harde spend-limits bij de aanbieders.
- **Betalen:** **Paddle** via Cashier Paddle (merchant of record, 5% + $0,50, regelt de EU-btw). Stripe direct betekent zelf OSS-btw afdragen. Lemon Squeezy migreert naar Stripe (onzeker).
- **Hosting:** Laravel Cloud in de EU-regio (vanaf ~$20–32/maand) of Forge + Hetzner (goedkoopst, volledig EU). Octane/FrankenPHP of voldoende FPM-workers: een worker staat 0,5–3 s te wachten op de aanbieder. Later eventueel een kleine proxy in Go/Rust of een Cloudflare Worker naast Laravel.

### 7.4 Kosten (1000 gebruikers × 30 min/dag = 900.000 min/maand)
| Aanbieder | Per maand | Per gebruiker |
|---|---|---|
| Voxtral / gpt-4o-mini-transcribe | $2.700 | $2,70 |
| Scribe v2 | $3.300 (+$750 keyterms) | $3,30–4,05 |
| gpt-transcribe | $4.050 | $4,05 |
| Deepgram (met training-opt-out) | ~$7.700–9.400 | ~$8–9 |

30 min per dag is een zware gebruiker; de mediaan ligt waarschijnlijk op 5–10 min.

**Voorbeeldrekening bij €12/maand incl. btw:**

| Post | Bedrag |
|---|---|
| Prijs incl. btw | €12,00 |
| Na btw | €9,92 |
| Paddle | −€1,05 |
| STT (zware gebruiker) | −€3,50 |
| **Marge** | **~€5** |

**Concurrenten:**

| Product | Prijs |
|---|---|
| Wispr Flow | $15/maand |
| Aqua Voice | $10/maand |
| Superwhisper | ~$8,50/maand of $250 lifetime |
| VoiceInk | $25–49 eenmalig (lokaal) |
| BridgeVoice | $50/maand bundel |

### 7.5 Privacy en AVG (kort)
- Bij B2C ben jij verwerkingsverantwoordelijke en zijn de STT-aanbieder en de hoster verwerkers. DPA's afsluiten.
- **De privacyverklaring moet bevatten:**
  - welke gegevens (stem, tekst, metadata);
  - grondslag (overeenkomst);
  - bewaartermijnen (audio: niet opgeslagen);
  - retentie en training bij de aanbieder;
  - ontvangers met land;
  - **doorgifte buiten de EER** (DPF of SCC's);
  - rechten en het klachtrecht bij de AP.
- Bij BYOK: vermelden dat audio rechtstreeks naar de gekozen aanbieder gaat.
- Stem is geen biometrisch gegeven zolang je er niemand mee identificeert. Een kleine DPIA is verstandig.
- Wil je volledig in de EU blijven, dan zijn Mistral, Speechmatics of Gladia de eenvoudigste AVG-positie.

---

## 8. Distributie

### 8.1 macOS
- **Apple Developer Program: $99/jaar.** Developer ID Application-certificaat (alleen de Account Holder kan dat aanmaken).
- **Notarization:** Tauri roept zelf `notarytool` en `stapler` aan. In CI met een **App Store Connect API key** (`APPLE_API_ISSUER`/`APPLE_API_KEY`/`APPLE_API_KEY_PATH`). **De DMG zelf wordt niet genotariseerd**; Epicenter doet dat als extra stap.
- **TCC en dev-builds:** TCC herkent een app aan zijn *designated requirement*.
  - Ad-hoc-gesigneerd (`-`) betekent dat de requirement de cdhash is, en die verandert bij elke build. Je verliest dan de Accessibility-permissie.
  - **Oplossing: altijd lokaal signeren met één vaste identity.** Je hebt al een Apple Development-certificaat (team C5HFB928WR) dat de Swift-app hiervoor gebruikte.
  - Test de permissie-flow met een gebundelde `.app`, niet met `tauri dev`. In dat geval krijgt Terminal of je IDE de permissie.
- **Dylibs** (bijv. ONNX Runtime) via `bundle.macOS.frameworks` en niet via `resources`. Resources worden niet gesigneerd, en dat breekt de notarization. Rpath `@executable_path/../Frameworks`.
- **Intel:** bouw native op `macos-15-intel` (beschikbaar tot augustus 2027). Universal is lastig met native C/C++-dependencies. Microsoft levert geen ONNX Runtime meer voor x86_64-mac; dat is nog een argument voor ggml.

### 8.2 Windows
- **Azure Artifact Signing** ($9,99/maand) is in de EU **alleen voor organisaties**. Particulieren moeten in de VS of Canada zitten. Of een eenmanszaak door de validatie komt, is niet zeker.
- **Alternatief:** een OV-cloudcertificaat (Certum SimplySign, SSL.com eSigner) vanaf ~$115/jaar. Sinds maart 2026 maximaal 460 dagen geldig. **EV geeft geen SmartScreen-voordeel meer.**
- NSIS-installer met `currentUser` (updates zonder UAC). WebView2 met `downloadBootstrapper`. Bundel de VC-runtime app-local.
- Voor whisper.cpp/ggml in CI: long paths aan, Vulkan SDK en SPIRV-Headers, `CMAKE_POLICY_VERSION_MINIMUM=3.5`, `GGML_NATIVE=OFF`.

### 8.3 Linux
- AppImage, deb en rpm, gebouwd op **Ubuntu 22.04**. `LINUXDEPLOY_EXCLUDED_LIBRARIES='libvulkan.so*;libwayland-client.so*'`.
- Flatpak is voor hotkeys en injectie niet aan te raden voor v1 (portals en brede device-rechten, Flathub-review).

### 8.4 Updater
- `tauri signer generate` → `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` in CI. **Private key kwijt = nooit meer updates.** Dus backup.
- `createUpdaterArtifacts: true`. `tauri-action@v1` met `uploadUpdaterJson: true` maakt `latest.json` (let op: de hele file wordt gevalideerd).
- **Statisch op GitHub Releases** is gratis, maar heeft geen kanalen en werkt niet met een private repo. **Een eigen endpoint (Laravel of R2)** geeft kanalen beta/stable en staged rollouts (contract: 204 = geen update, 200 = JSON).
- In de app: `check()` → `downloadAndInstall(progress)` → `relaunch()` (capabilities `updater:default` en `process:allow-restart`).

### 8.5 CI
Een GitHub Actions-matrix:
- `macos-latest` (arm64)
- `macos-15-intel`
- `windows-latest`
- `ubuntu-22.04`

Plus Swatinem/rust-cache (niet voor releases, zoals bij Handy) en `tauri-action@v1`. Buildtijd ~15–30 min per target met native ASR (schatting). Een volledige workflow werk ik uit in de distributiefase van de bouw.

**Kosten per jaar:**

| Scope | Kosten |
|---|---|
| Alleen macOS | ~$99 |
| + Windows-signing | ~$220–500 |
| GitHub Actions | Gratis bij een public repo; bij een private repo zijn macOS-minuten duur |

---

## 9. Vergelijkbare open-source apps

| Project | Stack | Licentie | Wat we ervan leren |
|---|---|---|---|
| **Handy** (cjpais/Handy) | Tauri 2 + Rust + React | **MIT** | Bijna alles: zie §3. Coordinator-state-machine met tests, recorder met rtrb, model manager (resume, sha256, idle-unload), overlay-recepten per OS, layout-bewuste plak-toets, Secure Input-workaround, tauri-specta en zustand. |
| handy-keys / transcribe-cpp / transcribe-rs | Rust-crates | MIT | Direct als dependency bruikbaar |
| tauri-nspanel | Rust-crate | MIT/Apache | NSPanel-overlay |
| OpenWhispr | Electron + native helpers | MIT | Windows fast-paste (modifiers loslaten, terminal-detectie, focus via `AttachThreadInput`). Fn-listener die tijdelijk `AppleFnUsageType` omzet. |
| Vibe | Tauri 2 + React | MIT | **Inferentie in een sidecar-proces**: een GPU-crash of SIGILL doodt alleen het kind |
| Epicenter / Whispering | Tauri 2 + Svelte | **AGPL** | Alleen ideeën: volledige pasteboard-snapshot, ConcealedType, media pauzeren tijdens een opname, DMG apart notariseren |
| VoiceInk | Swift | **GPL** | Alleen ideeën: `CGEventSource(.privateState)`, TransientType plus eigendomscheck, hybride PTT/toggle |
| Voicetypr, OpenLess, Tambourine | Tauri | AGPL | Niet bekeken op code |

**Licentieregel:** van MIT/Apache mogen we code overnemen, met behoud van copyright in `THIRD_PARTY_NOTICES` en in de header van overgenomen bestanden. Van GPL/AGPL geen code, alleen technieken en API-namen.

---

## 10. Bronnen (selectie)

**Tauri:**
- https://github.com/tauri-apps/tauri/releases
- https://github.com/tauri-apps/plugins-workspace
- https://v2.tauri.app/security/capabilities/
- https://v2.tauri.app/develop/calling-frontend/
- https://v2.tauri.app/plugin/updater/
- https://v2.tauri.app/distribute/sign/macos/
- https://v2.tauri.app/distribute/sign/windows/
- https://github.com/tauri-apps/tauri-action/releases/tag/action-v1.0.0
- Issues #9198, #14420, #12534, #14102, #15471, #15947
- Stronghold-deprecation: https://github.com/tauri-apps/plugins-workspace/issues/3494

**Referentie-apps:**
- https://github.com/cjpais/Handy (issues #502, #1279, #1283, #1578, #1620, #1827, #1902, #1930)
- https://github.com/handy-computer/handy-keys
- https://github.com/handy-computer/transcribe.cpp
- https://github.com/ahkohd/tauri-nspanel
- https://github.com/OpenWhispr/openwhispr
- https://github.com/thewh1teagle/vibe

**BridgeVoice:**
- https://www.bridgemind.ai/bridgevoice
- https://www.bridgemind.ai/changelog/bridgevoice/v4-1-18 (t/m v2-2-50)

**Crates:**
- https://crates.io/crates/cpal
- https://github.com/HEnquist/rubato
- https://github.com/pykeio/earshot
- https://github.com/pykeio/ort
- https://github.com/altunenes/parakeet-rs
- https://github.com/1Password/arboard
- https://github.com/enigo-rs/enigo/issues/480
- https://crates.io/crates/keyring

**Modellen:**
- https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3
- https://huggingface.co/nvidia/canary-1b-v2
- https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b
- https://huggingface.co/ggerganov/whisper.cpp
- https://arxiv.org/abs/2509.14128
- https://arxiv.org/abs/2212.04356
- https://arxiv.org/abs/2402.08021 (Whisper-hallucinaties)

**Cloud:**
- https://developers.openai.com/api/docs/pricing
- https://elevenlabs.io/pricing/api
- https://mistral.ai/news/voxtral-transcribe-2/
- https://deepgram.com/pricing
- https://www.assemblyai.com/pricing
- https://console.groq.com/docs/speech-to-text
- https://artificialanalysis.ai/speech-to-text/non-streaming
- https://www.paddle.com/pricing
- https://laravel.com/cloud/pricing

**Apple en Microsoft:**
- TN3127 https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements
- https://developer.apple.com/forums/thread/730043
- https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
- https://learn.microsoft.com/en-us/azure/artifact-signing/faq
