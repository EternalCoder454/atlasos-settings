# Settings (Telamon OS)

Rust + Qt 6.11 + Kirigami (CXX-Qt) settings app for Telamon OS, a Fedora Kinoite
44 bootc image (repo `~/Documents/Projects/AtlasOS/AtlasOS`). It replaces KDE
System Settings: its own pages over the system's services, the KCMs it has no
page for under Other Plasma Settings, and System Settings' entry points
(`systemsettings`, KCMLauncher, the `kcm_*.desktop` launchers) at cutover.
Read `docs/DESIGN.md` first: it fixes the layout, the threading rule, what is
trusted, who owns what, and the budgets. Change it only together with the
code that implements the change.
The plan and roadmap are the Atlas Notes notes "AtlasOS/Settings/Plan" and
"AtlasOS/Settings/Roadmap".

The stack, build and look are Atlas Monitor's
(`~/Documents/Projects/AtlasOS/AtlasOS Monitor`). When in doubt, do what it
does, except for what atlas-framework provides (startup, settings file,
logging, crash reports), which Settings takes from there.

## Hard rules

- **Build and test inside the `fedora:44` dev container**, never on the host:
  `scripts/dev.sh <command>`. The repo is at `/src`; all build output goes to
  `/work` (`~/.cache/claude-builds/telamon-settings` on the host), never into
  the repo or `/tmp`. Use a separate target dir per agent or task
  (`CARGO_TARGET_DIR=/work/target/<name> scripts/dev.sh ...`).
- **Intensive jobs go through the machine-wide queue**:
  `~/.claude/heavy/run.sh <name> "$PWD" '<command>'` for container builds,
  full test runs, RPMs and benchmarks, run in the background; read the log
  it prints. Check `free -h` first.
- **Never touch the real system's settings.** Tests talk to python-dbusmock
  services on a private bus (`crates/settings-sys/tests/common`), never the
  host's system or session bus. Smoke runs (`scripts/smoke.sh`) use a session
  bus that can't start services, Xvfb, and `XDG_*_HOME` under `/work/smoke`.
  Real end-to-end tests happen in the Telamon OS test VM, which the Telamon OS
  session runs.
- **Use the system's services; never reimplement them.** Every privileged
  change goes through the service that owns it (NetworkManager,
  AccountsService, timedated, localed, ...), which asks polkit itself. No root
  helper, polkit action or setuid binary of our own.
- **Everything from outside is untrusted**: launch arguments (any app can
  start us), D-Bus replies (SSIDs, device and user names), KCM metadata.
  Validate it in Rust (`crates/settings-registry`, `crates/settings-sys`) with
  limits; show it as plain text, never RichText. Programs start from argv
  vectors through `cpp/launcher.cpp`, never through a shell.
- **The GUI thread never blocks on D-Bus.** System calls run on worker
  threads with timeouts (`settings_sys::bus`); results come back with
  `qt_thread().queue`.
- **Telamon.Ui is the installed `telamon-ui` package** from atlas-framework
  (`~/Documents/Atlas Framework`, read-only from here). Never copy Telamon.Ui
  controls into this repo: ask the "AtlasOS Framework" session. Use only API
  that exists at the pinned release (`fw-src/api/telamon-ui.api` at the tag,
  not "Since 1.5.0" members).
- **The page registry is the one list.** Sidebar, search, deep links, the
  KCM map and Other Plasma Settings read `crates/settings-registry`; never list pages
  or KCM names anywhere else.
- **Don't edit the Telamon OS image.** Hand the Telamon OS session an RPM or a
  commit to pin (`atlas-apps.lock`). Until cutover the desktop file stays
  `NoDisplay=true` and nothing of KDE's is replaced.
- Tests assert invariants and use fixtures (`crates/*/tests/fixtures`),
  never this machine's KCMs or services.
- Commits are authored as
  `EternalHell <77252745+EternalCoder454@users.noreply.github.com>`. Commit
  only the paths you own (`git commit -- <paths>`). Don't push unless the
  lead asked.
- Licence: MIT. App ID `net.eterneon.telamon.settings`. Wording follows KDE:
  Title Case buttons and titles, US spelling.

## Commands

| Task | Command (from the repo root on the host) |
|---|---|
| Format | `scripts/dev.sh cargo fmt --all --check` |
| Lint | `scripts/dev.sh cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Tests | `scripts/dev.sh cargo test --workspace --locked` |
| App build | `scripts/dev.sh bash -c 'cmake -S apps/telamon-settings -B /work/cmake/dev -G Ninja && cmake --build /work/cmake/dev'` |
| Smoke run | `SMOKE_OUT=/work/smoke/<name> scripts/dev.sh scripts/smoke.sh [app args]` (screenshot and log in `~/.cache/claude-builds/telamon-settings/smoke/<name>`) |
| RPM | `podman run --rm --security-opt label=disable -v "$PWD":/src -v <framework rpms>:/telamon-rpms:ro -e TELAMON_LOCAL_RPMS=/telamon-rpms -v telamon-cargo:/root/.cargo/registry -v telamon-cargo-git:/root/.cargo/git -e CARGO_HOME=/root/.cargo registry.fedoraproject.org/fedora:44 /src/packaging/build-rpm.sh /src/out` |
| Telamon app checks | `git -C ~/Documents/Atlas\ Framework archive v2.0.0 tools ui \| tar -x -C <dir>`, then `<dir>/tools/lint-app.sh apps/telamon-settings` and `<dir>/tools/check-app-names.sh apps/telamon-settings` |

`<framework rpms>` is the out dir of atlas-framework's `packaging/build-rpm.sh`
(here `~/.cache/claude-builds/telamon-settings/fw-rpms-2.0.0`): no repository
has telamon-ui. `scripts/dev.sh` builds `localhost/telamon-settings-dev:44` on
first use, which needs `TELAMON_LOCAL_RPMS=<dir>` holding them.

Without `TELAMON_REQUIRE_DBUSMOCK=1` the `settings-sys` service tests skip when
`dbus-daemon` or python-dbusmock is missing; CI and `scripts/dev.sh` set it, so a
skip there is a failure.

## Moving the atlas-framework pin

1. Change `tag` in `Cargo.toml`, then
   `scripts/dev.sh cargo update -p telamon-framework-ui`.
2. Move the Telamon app checks job in `.github/workflows/ci.yml`:
   `app-checks.yml@<the tag's commit> # vX.Y.Z` and `framework-ref: vX.Y.Z`
   (`git ls-remote https://github.com/EternalCoder454/atlas-framework 'refs/tags/vX.Y.Z^{}'`).
   The framework job reads the tag from `Cargo.toml` and fails when the two
   disagree.
   When the app uses something new in Telamon.Ui, also `ui:` in `src/lib.rs`
   and `telamon-ui >=` in the spec (Requires and BuildRequires).
3. Refresh `crates/settings-registry/tests/fixtures/symbols-telamon-ui-*.txt`
   from that release's `symbols.txt`.
4. Rebuild the dev image against that release's RPMs.
5. Commit `Cargo.toml` and `Cargo.lock` together.
