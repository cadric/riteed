Repo-kontekst:

Riteed er en GNOME-only Rust app med gtk4, libadwaita, sourceview5, gettext, GSettings og Flatpak-first distribution. Runtime Rust forbyder unsafe. Flatpak-manifestet forventes at have meget smalle finish-args: Wayland, eventuel kontrolleret fallback-X11 hvis manifestet faktisk viser det, og ingen unødvendig network/share/home/host filesystem permission. Vurder faktisk manifest, ikke antagelser.

AGENTS.md er en hård governance-kontrakt, ikke kun dokumentation. Repoet har lokale policy JSON-filer og validators. Markér alle steder hvor policy-intention findes i prosa/JSON, men ikke håndhæves maskinelt.

Primære trust boundaries:

- brugeråbnede tekst- og Markdown-filer
- Git repositories og Git output parsing
- /app/bin/git subprocess-boundary i Flatpak
- forskel mellem host Git i tests og bundled /app/bin/git i Flatpak
- Flatpak sandbox/portal-adfærd
- GitHub Pages Flatpak remote og GPG/OSTree signing
- GTK/TextBuffer/TextTag/TextMark lifecycle
- signal handlers, async callbacks og UI-state
- save/session/multi-window/concurrent edit flows

Læs først i denne rækkefølge:

1. AGENTS.md
   Behandl den som autoritativ kontrakt. Notér især den differentierede fil-limit (600 produktion / 800 test / 720 waiver-cap), async Gio FS-regel, no-co-author commit policy, no unsafe runtime Rust-policy og alle andre regler der påvirker auditens vurdering.

2. policy/*.json, tools/policy_check.py, tools/coverage_check.py
   Find intention vs enforcement gaps. Særligt: parser/untrusted-input krav om property/fuzz tests må ikke kun eksistere som policy-intention uden maskinel håndhævelse.

3. tools/checks/dependency_preflight.py
   Vurder gtk-rs stack-pinning, exact safe/sys version pairs, Flatpak cargo-sources integrity, lockfile/policy/cargo-source edge cases og bypassmuligheder.

4. app/src/lib.rs og app/src/app.rs
   Forstå app entry, GApplication wiring, HANDLES_OPEN, multi-window/multi-instance og open-file flow.

5. app/build-aux/io.github.cadric.Riteed.yml
   Auditér finish-args, bundled git, build sources, cargo-sources.json, patches og sandbox assumptions.

6. .github/workflows/*
   Auditér native CI, validate jobs, stress/fuzz jobs, Flatpak build, signing, deploy til GitHub Pages, cleanup, concurrency og secret exposure.

7. app/fuzz/, proptest tests, riteed-stress og ASan/Valgrind scripts
   Vurder faktisk fuzz/stress coverage. Fokuser især på gap mellem fuzz harnesses og reel app-input gennem GTK/UI/render/save/git flows.

Derefter auditér følgende områder i prioriteret rækkefølge.

A. Unsafe/FFI

- List alle unsafe blocks i app/src/.
- Hver unsafe forekomst skal have en konkret SAFETY-kommentar.
- Vurder om SAFETY-kommentaren faktisk beviser invarianten.
- gtk-rs bindings kræver sjældent unsafe; hver forekomst er høj-signal.
- Hvis runtime Rust-policy siger unsafe er forbudt, vurder om forekomsten er policy-brud, test-only undtagelse eller legitimeret boundary.

B. GTK/GObject lifecycle

Auditér især:

- signal handler lifetimes
- disconnect/cleanup af signal handlers
- stale callbacks efter window/buffer/document close
- glib::clone! usage
- @strong mod ApplicationWindow, buffer, document model, TextView eller controller
- @weak / @weak-allow-none correctness
- ref-cycles mellem window, buffer, document, controllers og async closures
- GMainContext/UI-thread assumptions
- async callbacks der opdaterer lukket/stale UI
- TextBuffer/TextTag/TextMark lifetimes
- preview/search/minimap/diff tags
- shared TextBuffer tag manipulation

Find særligt om preview/search/minimap/source-control/diff kan:

- skabe ref-cycles
- lække TextTags/TextMarks
- efterlade stale callbacks
- ændre is_modified utilsigtet
- påvirke undo stack
- kalde irreversible undo APIs
- ændre brugerens dirty-state via ren visualisering
- race mod buffer edits, document URI change eller SCM refresh

C. Source Control, Git boundary og subprocess

Auditér app/src/git_process.rs og relaterede moduler.

Se efter:

- non-UTF-8 paths
- paths med newline, tab, quotes, leading dash, control chars
- symlinks
- submodules
- detached HEAD
- unborn repos
- missing identity
- index.lock
- worktrees
- filters/EOL conversion
- large repos
- renamed/copied files
- deleted files
- conflicted files
- weird porcelain v2 entries
- pathologically long paths
- malicious repository contents
- difference mellem tests med /usr/bin/git og Flatpak prod med /app/bin/git
- Command::new / Command::arg usage
- ingen shell-injection via shell
- argument injection via user-controlled args
- cwd handling
- environment handling
- locale/output encoding
- timeout/cancellation behavior
- process cleanup

Vurder specifikt om appen antager UTF-8 på steder hvor Git ikke garanterer det.

D. Parser/fuzz/property coverage

Auditér:

- custom Markdown parser
- frontmatter split
- Git porcelain parser
- diff compute
- unsupported scanner
- alle proptest/fuzz harnesses
- riteed-stress
- scheduled/manual stress workflows

Du skal ikke nøjes med at konstatere at fuzz findes. Vurder:

- om harnesses matcher reel app-input
- om parseren fuzzes med samme normalization/decoding som appen bruger
- om GTK/render/save path er dækket eller kun pure-Rust lag
- om frontmatter edge cases rammer faktisk document open/save flow
- om diff fuzz tester samme inputtyper som Git boundary producerer
- om unsupported scanner kan påvirkes af store filer, binary-ish input eller invalid UTF-8
- om fuzz lockfile og CI faktisk holder sig synkroniseret med workspace dependencies
- om corpus/crash artifacts håndteres fornuftigt
- om scheduled 30-min jobs kan give falsk tryghed uden boundary coverage

Lav gerne minimale lokale stress-/fuzz-inputs hvis det er muligt, men rapportér kun fund med reproducerbare trin.

E. Flatpak sandbox og portals

Auditér app/build-aux/io.github.cadric.Riteed.yml.

For hver finish-arg:

- forklar hvorfor den findes
- vurder om den er nødvendig
- vurder blast radius
- foreslå smallere alternativ hvis muligt

Se specifikt efter:

- --filesystem=host, home, xdg-* eller andre brede FS-permissions
- --share=network
- --socket=*
- --talk-name / --own-name
- D-Bus permissions
- fallback-X11 hvis relevant
- Wayland-only assumptions
- portal usage til filvalg/open-uri
- direkte filesystem access hvor portal burde bruges
- om appen kan tilgå Git repos uden at bryde sandbox-intent
- om /app/bin/git er korrekt bundled, pinned og isoleret
- om sandbox escape assumptions findes i kode eller docs

F. Supply chain, dependencies og cargo-sources

Auditér:

- Cargo.lock
- cargo-sources.json generation og integritet
- Flatpak source pinning
- git dependencies
- floating refs
- path dependencies uden for workspace
- patched sourceview5 under build-aux/cargo-patches/sourceview5
- gtk4/gtk4-sys/libadwaita/sourceview5 safe/sys version pairs
- tools/checks/dependency_preflight.py bypassmuligheder
- mismatch mellem lockfile, policy JSON og generated cargo-sources
- fuzz lockfile sync
- om flatpak-builder faktisk verificerer sha256
- om dependency_preflight kan narres af mærkelige package names, duplicated crates, workspace state, vendored source eller stale generated files

Forklar hvorfor enhver patch findes, hvad den ændrer, og om patchen skaber FFI/API-lifetime-risiko.

G. GPG/OSTree signing og GitHub Pages Flatpak remote

Auditér alle release/deploy/signing workflows.

Se efter:

- kompromitteret GitHub secret blast radius
- kan angriber pushe signeret evil update til eksisterende installs?
- key rotation story
- revocation story
- rollback story
- parallel releases mod samme GitHub Pages branch
- concurrency groups
- partial deploy
- build failure mid-signing
- tempfile cleanup
- GNUPGHOME cleanup
- gpg-agent lifetime/state
- secret echo/log leakage
- artifact poisoning mellem jobs
- permissions: contents/packages/pages/id-token
- branch protection assumptions
- untrusted PR access til secrets
- reproducibility eller manglende reproducerbarhed
- provenance/SBOM hvis eksisterende

H. Policy intent vs enforcement

Lav en separat tabel med alle policy-regler hvor:

- intention findes i AGENTS.md, policy/*.json eller docs
- enforcement mangler eller er svag
- enforcement findes men kan bypasses
- enforcement tester kun happy path
- validator kan omgås med filnavne, generated files, stale lockfiles, workspace layout eller CI-matrix gaps

Dette er governance-gæld. Behandl det som auditfund, ikke som dokumentationskommentar.

I. Save/session/concurrency

Auditér:

- HANDLES_OPEN
- multi-window behavior
- samme fil åbnet i to vinduer
- simultaneous saves
- autosave/session restore hvis relevant
- dirty-state tracking
- async refresh races
- SCM/minimap/diff callbacks efter buffer change
- document URI change
- stale generation/fingerprint checks
- save while source-control refresh kører
- source-control/minimap/diff må ikke ændre is_modified eller undo state
- file locking strategy eller fravær af samme
- async Gio FS-reglen fra AGENTS.md
- std::fs:: / tokio::fs:: i app/src/
- sync FS på UI thread

J. GSettings

Auditér alle settings-moduler og schema-filer.

Se efter:

- sane defaults
- type changes mellem versioner
- removed keys
- migration story
- invalid stored values
- settings der påvirker security/sandbox/input handling
- schema install/compile i Flatpak
- mismatch mellem Rust constants og schema keys

K. GLib/Gtk criticals and warnings

Selvom native CI kører med G_DEBUG=fatal-criticals, auditér stadig:

- g_critical!
- g_warning!
- log::warn!/error! hvor invariant burde være fatal
- GTK warnings der kun opstår ved UI flows ikke dækket af tests
- om CI faktisk kører de flows der ville trigge criticals
- om ASan/Valgrind smoke scripts dækker relevante GTK lifecycle paths

Kommandoer og greps du som minimum bør bruge eller ækvivalent inspicere:

- rg -n "unsafe" app/src
- rg -n "glib::clone!|@strong|@weak|@weak-allow-none|clone!\(" app/src
- rg -n "connect_|disconnect|SignalHandlerId|handler_block|handler_unblock" app/src
- rg -n "TextBuffer|TextTag|TextMark|create_tag|apply_tag|remove_tag|set_modified|is_modified|begin_irreversible|undo" app/src
- rg -n "Command::new|Command::arg|std::process|gio::Subprocess" app/src
- rg -n "std::fs::|tokio::fs::|File::open|read_to_string|write\(" app/src
- rg -n "g_critical|g_warning|critical|warning|fatal-criticals" .
- rg -n "filesystem=|share=|socket=|talk-name|own-name|device=|persist=" app/build-aux .github
- rg -n "gpg|GNUPGHOME|ostree|flatpak|pages|deploy|secret|concurrency" .github app/build-aux tools
- rg -n "cargo-sources|Cargo.lock|sourceview5|gtk4-sys|libadwaita|patch" .

Kør tests/checks hvis miljøet tillader det:

- python tools/policy_check.py
- python tools/coverage_check.py
- python tools/checks/dependency_preflight.py
- cargo test --workspace --locked
- cargo fuzz list
- kort smoke-run af relevante fuzz targets hvis toolchain findes
- eksisterende stress job/scripts hvis de kan køres lokalt uden at ændre repoet
- ASan/Valgrind smoke scripts hvis miljøet tillader det

Hvis en kommando ikke kan køres, markér den som “ikke kørt” og forklar hvorfor. Erstat ikke manglende eksekvering med antagelser.

Outputformat:

Start med:

1. Repo/commit audited
   - commit hash
   - branch
   - dato
   - hvilke kommandoer/checks blev kørt
   - hvilke blev ikke kørt

2. Executive summary
   - 5-10 korte punkter
   - vigtigste reelle risici
   - vigtigste governance gaps
   - vigtigste områder der ser robuste ud

3. Findings table

Brug denne tabelstruktur:

| ID | Severity | Area | File/line | Finding | Evidence | Impact | Repro/trigger | Recommendation | Confidence |
|----|----------|------|-----------|---------|----------|--------|---------------|----------------|------------|

Severity:

- Critical: remote/supply-chain compromise, sandbox escape, silent malicious update, reliable data loss på almindelige flows
- High: signifikant trust-boundary bypass, user-controlled input crash/data corruption, unsafe/FFI invariantbrud, broad sandbox permission uden stærk begrundelse
- Medium: plausible race/leak/stale callback, parser edge case, policy enforcement gap med realistisk bypass, CI gap der kan skjule regressions
- Low: hardening, observability, minor lifecycle risk, weak docs where enforcement is mostly correct
- Info: positive assurance, verified invariant, non-issue worth documenting

For hvert fund:

- Giv konkrete fil-/linjereferencer.
- Citer relevant kode eller workflow-snippet kort.
- Forklar hvorfor det er et problem i Riteeds konkrete trust model.
- Giv en anbefalet rettelse der passer til GNOME/gtk-rs/Flatpak, ikke en generisk anbefaling.
- Angiv confidence: High/Medium/Low.
- Skeln klart mellem verificeret fund, mistanke og ikke-verificeret risiko.

4. Deep-dive sections

Lav separate afsnit for:

- GTK/GObject lifecycle
- Git boundary
- Parser/fuzz/stress coverage
- Flatpak sandbox
- Supply chain/dependency pipeline
- GPG/OSTree/GitHub Pages signing
- Policy intent vs enforcement
- Save/session/concurrency
- GSettings
- CI fatal-criticals/ASan/Valgrind coverage

5. Positive assurance

List ting du faktisk har verificeret som gode, f.eks.:

- ingen unsafe blocks fundet, hvis sandt
- finish-args er smalle, hvis sandt
- cargo-sources checksums verificeres, hvis sandt
- clone! usage er overvejende weak, hvis sandt
- fuzz targets matcher bestemte boundaries, hvis sandt

Men skriv kun positive assurance når det er baseret på konkret inspektion.

6. Top 10 next actions

Prioritér efter sandsynlighed × blast radius. Hver action skal være konkret og kunne omsættes til issue/PR.

Vigtige auditregler:

- Antag ikke at dokumentation er korrekt. Verificér mod kode, manifest og workflows.
- Antag ikke at policy håndhæves fordi den står i AGENTS.md eller policy JSON.
- Antag ikke at fuzzing dækker en boundary fordi der findes et fuzz target med lignende navn.
- Antag ikke at Flatpak sandbox er sikker fordi manifestet ser lille ud; vurder portals, subprocesses og Git repo access.
- Antag ikke at tests med host Git repræsenterer /app/bin/git i Flatpak.
- Antag ikke at GTK warnings fanges hvis CI ikke udøver relevante UI paths.
- Prioritér konkrete fund over brede anbefalinger.
- Undgå stilkommentarer.
- Undgå spekulation uden at markere den som spekulation.
- Hvis du finder noget alvorligt, stop ikke auditten; fortsæt og lav fuld prioriteret rapport.
