# Settings: security

The threat model of Telamon Settings: what it protects, who it defends against,
the rule each entry point follows, and the test that keeps the rule true.
`docs/DESIGN.md` ("Trust") says what is trusted in one list; this file says
why, and what is left. Change it together with the code it describes: some of
the tests below read the QML and fail when a rule here is skipped.

Report a vulnerability privately to the maintainer through GitHub's "Report a
vulnerability" on the repository (Security tab), not in a public issue.

## What Settings is, security-wise

- It runs as the signed-in user, in the session, and has **no privilege of its
  own**: no setuid file, no root helper, no polkit action, no service of its
  own. Every privileged change is a D-Bus call to the service that owns the
  thing (NetworkManager, AccountsService, timedated, localed, hostnamed,
  firewalld through systemd, fwupd, telamon-system-helper, UPower, ...), and
  that service asks polkit itself. A bug in Settings can therefore not do more
  than the user, or the user plus one polkit prompt they accept, could do.
- It replaces KDE System Settings, so other programs start it, by name
  (`systemsettings kcm_x`), by D-Bus (the Launcher's deep links, Plasma's
  KCMLauncher) and with arguments. Anything in the session can do that.
- It shows text it did not write: SSIDs, user and device names, wallpaper and
  KCM metadata, release notes, firmware text, crash reports.
- It holds secrets briefly: a new account's password, a Wi-Fi or hotspot
  password.

## Who we defend against

| Attacker | What they can reach | Defended? |
|---|---|---|
| **A hostile program in the session**, typically a sandboxed (Flatpak) app that was given a D-Bus name to talk to, or a compromised app | Settings' D-Bus names and command line, files it can write in the user's folders | Yes: this is the main model. Everything arriving from it is validated (see 1 to 3) |
| **Another local user** | The names they set (full name, user name), pictures they own, shared folders, the system bus | Yes: their text is shown as plain text, their files are checked before use |
| **Remote data** | SSIDs, Wi-Fi/Bluetooth device names, update metadata and release notes, fwupd/LVFS text, DMI strings, the crash relay's answers | Yes: untrusted text (length, control characters, plain text only); links only https and only to allowlisted places |
| **A file planted in the user's config or data folders** (by an app that may write there) | Autostart entries, mimeapps.list, Flatpak overrides, wallpaper folders | Yes: sizes, types and link targets are checked before use or write |
| **The supply chain** | crates.io, the pinned git dependencies, the build container, CI actions | Partly: locked and pinned builds, `cargo-deny`, `cargo-audit` (see 8) |

Out of scope, because it is not Settings' to stop: code running as the same
user outside any sandbox (it can already do everything Settings can, and read
the user's files); a malicious or compromised OS image; physical access; bugs
in the services Settings calls (NetworkManager, AccountsService, polkit, KDE
tools); and what the pinned `atlas-framework` and `atlasos-updater` crates do
inside their own boundaries (we review how Settings calls them, below).

## 1. Hostile D-Bus callers and the command line

Entry points, all on the session bus or the command line:

| Entry | Who can call it | What it can ask for |
|---|---|---|
| `net.eterneon.telamon.settings` `org.freedesktop.Application.Activate` / `Open` | any session process | raise the window (`Open` ignores the URIs) |
| `...Application.ActivateAction("open", [link])` | any session process | a **page and setting of the registry**, nothing else |
| `...Application.ActivateAction("open-app", [id])` | any session process | the Apps page at App Permissions |
| `org.kde.KDBusService.CommandLine(args)` | any session process; this is how a second `telamon-settings ...` start reaches the first | what the command line can: a page, a setting, a search, a KCM by name |
| `net.eterneon.atlas.settings` (the old name, for one more release) | any session process | the same as the new name's `Activate`, `Open` and `ActivateAction`, through the same check |
| `telamon-settings ...`, `systemsettings ...` | the user's own processes | as `CommandLine` |

Rules:

- **Deep links open registry pages only.** `launch::action_args` takes a link
  from the search index (`page` or `page/setting`, `[a-z0-9][a-z0-9-]*`
  segments, at most 128 bytes) or a desktop file ID (`[A-Za-z0-9._+-]`, at
  most 200 bytes) and nothing else; any other text is refused and the window
  only comes forward. The text is never split into arguments: the link is
  checked as a whole, and what is handed on can only be a page. This applies
  to **both** D-Bus names; before 0.5.0 the old name split the text on spaces
  and fed it to the command-line parser, so `--kcm` and `--search` got in.
  *Tests:* `fuzz_launch.rs` (`deep_links_only_open_pages`,
  `a_deep_link_cannot_carry_the_options_a_command_line_can`),
  `legacyservice_test.cpp`, `actionservice_test.cpp`.
- **The command line (and `CommandLine`) is validated, in Rust only**
  (`launch::parse`): at most 16 arguments of 256 bytes (`--args` 1 KiB), no
  control characters, unknown options and unknown pages and settings refused,
  what was refused shown as plain text (escaped, 64 characters). The only way
  to start another program is a **KCM name**: `kcm`, an optional `_`, a letter
  or digit and up to 64 of `[A-Za-z0-9_-]`; `.desktop` and Plasma's plugin
  path form (`plasma/kcms/<folder>/<name>`, no `.`, `..` or empty parts) are
  stripped to the name; a name Settings has a page for opens the page. The
  KCM must be installed (the KCM catalog) before it is started. Starts are
  limited to 5 in 10 s and a repeat within 2 s is dropped.
  *Accepted:* a session process can thus open any installed KCM, with
  `--args` text, in `kcmshell6`; it could run `kcmshell6` itself. Flatpak's
  bus filter is what keeps sandboxed apps from talking to Settings at all
  unless they were given its name.
- **The activation token** of a call is only passed to the window system
  (`KWindowSystem`), never to a command.
- The object the old name exports has exactly three methods (`Activate`,
  `Open`, `ActivateAction`), no signals and none of `QObject`'s slots.
  *Test:* `legacyservice_test.cpp` (`exportsOnlyTheApplicationInterface`).

## 2. Starting programs: no argument injection

- Programs start from **argument vectors**, never through a shell:
  `KIO::CommandLauncherJob` from `Launcher::run` (C++), `exec` in the shim.
- `Launcher::run` takes only an **allowlist of program names**: `kcmshell6`,
  `plasma-apply-colorscheme`, `plasma-apply-wallpaperimage`,
  `plasma-apply-lookandfeel`, `gsettings`, `telamon-store`. The name is looked
  up in `/usr/bin` only (never `PATH`, so nothing in `~/.local/bin` runs in its
  place); a path, a shell or an interpreter is refused.
  *Test:* `launcher_test.cpp` (`onlyTheProgramsSettingsStartsAreRun`).
- **`kcmshell6`** gets the KCM name (which cannot start with `-`, see 1) and
  the module's text as **one** word, `--args=<text>`, so text starting with `-`
  stays text. *Tests:* `fuzz_plan.rs`, `fuzz_launch.rs`, `plan.rs`,
  `backend.rs` (`kcm_commands`), `scripts/ui-stress.sh` (shell syntax in
  `--args` runs nothing).
- **`plasma-apply-colorscheme` / `-lookandfeel` / `-wallpaperimage`** get names
  that are read back from the installed lists (a scheme must be one of the
  scheme files; a Global Theme or wallpaper must be one `lookAndFeels()` /
  `wallpapers()` offered, by exact ID), made of `[A-Za-z0-9._-]`, **starting
  with a letter or digit** so a folder named `--help` cannot become an option,
  and anchored with `\A...\z` (a regular expression's `$` lets a trailing
  newline through). Wallpaper image paths are absolute, have no control
  characters and no `'` (the tool builds a script around the name).
- **`xdg-open`** is never run. Links go through `Qt.openUrlExternally`, which
  Qt hands to the desktop (the OpenURI portal or `xdg-open`) as a URL, never
  assembled into a command by us (see 6).
- `Process`, `Qt.createQmlObject`, `eval`, `XMLHttpRequest`, `Qt.include` and
  web views are not used in the QML (checked by `qml_text.rs`).

## 3. The `systemsettings` shim

`/usr/bin/systemsettings` (crate `systemsettings-shim`) answers the command
line of KDE's `systemsettings`.

- It **parses no more than the registry does**: it builds the same
  `--kcm <raw>` request and asks `launch::parse` what it is. A KCM Settings has
  a page for goes to `/usr/bin/telamon-settings --kcm <name>`; any other valid
  name goes to `/usr/bin/kcmshell6 <name> [--args=<text>]`.
- **Absolute targets, no `PATH`, no shell**: `Command::new("/usr/bin/...").exec()`
  with the program name as `arg0`. It is not setuid; the environment is the
  caller's.
- Anything it does not understand is refused (status 2, one line, nothing
  started): non-UTF-8 arguments, more than 16 arguments, an unknown option, two
  modules, a name that is not a KCM name (`../../bin/sh`, `kcm_a;rm`, ...).
- *Tests:* `plan.rs` (examples), `fuzz_plan.rs` (property: whatever argv, the
  plan is a print, a refusal, or an exec of one of the two absolute programs
  with a plain KCM name and at most one `--args=` word), `process.rs` (the real
  binary), and the package's `%check` (`systemsettings '../../bin/sh'` must
  fail with status 2).

## 4. Secrets

- **Wi-Fi and hotspot passwords** go from the password field to Rust as a
  `QString` copy, into a `Secret` (not shown by `Debug`, overwritten when
  dropped), and from there only to NetworkManager in `AddAndActivateConnection`.
  NetworkManager stores them (root-only, in its connection files). **Settings
  never reads a secret back**: no `GetSecrets`, and `GetSettings` omits them.
  A connection that does not come up is deleted again.
- **VPN secrets** are not handled at all: Settings activates the saved
  connection, and NetworkManager asks the session's secret agent (Plasma's,
  which keeps them in KWallet) if it needs them.
- **Account passwords** are hashed in Settings with libcrypt (**yescrypt**,
  `$y$`, as Fedora stores them; SHA-512 with 100,000 rounds where libcrypt has
  no yescrypt), verified by hashing again, and only the **hash** goes to
  AccountsService (`SetPassword(hash, "")`); the clear text is never on the
  bus. The C string handed to `crypt` is overwritten afterwards and the Rust
  copy (`Secret`) too. The longest accepted password is 511 bytes (libcrypt's
  limit), refused before an account is created, not after.
- **Never logged**: error details come from the services' D-Bus error text,
  which does not carry the values sent; a test pins that `Secret` prints as
  `Secret(..)`; the crash reports' redaction is the framework's.
- **Fields are emptied** when their sheet closes or a join is tried
  (`...text = ""` for every `TelamonPasswordField`, which `qml_text.rs`
  insists on), so a password does not sit in a QML property after it was used.
- *Residual:* Qt's own copies of a typed string (the field, the `QString`
  that crossed into Rust) are freed by Qt, not wiped; and the hash can appear
  in the argument list
  (a hash, not the password), in the helper AccountsService runs.

## 5. Accounts (AccountsService)

- Every change is one of AccountsService's own polkit actions
  (`org.freedesktop.accounts.user-administration` for adding or removing
  users, making administrators and changing other people's data and
  passwords; `change-own-user-data` and `change-own-password` for oneself);
  Settings sends the interactive-authorization flag so the prompt can appear,
  and **adds no allow-rule of its own that could disagree with polkit**.
- Settings adds only: valid names (`[a-z_][a-z0-9_-]{0,31}`, full names
  without control characters, `:` or `,`), no deleting the account in use,
  and **no account without a password**: if setting the password fails after
  `CreateUser`, the new user is removed again.
- **Avatars.** The picture the person chose is looked at on a worker thread
  before AccountsService is asked to copy it (`accounts::check_picture`): the
  path is resolved (links followed), opened with `O_NONBLOCK|O_NOFOLLOW`
  (a pipe is not waited for), must be a **regular file**, not empty, **at most
  1 MB**, and a **PNG, JPEG, GIF or WebP by its first bytes**, whatever it is
  called. SVG is not accepted (a document is not a picture the sign-in screen
  should draw). The path handed to the service is the *resolved* one, so a link
  that is changed after the check does not change what was checked; the
  service then copies it itself, as the user, with its own checks. The reason a
  file is refused is shown in words. *Tests:* `accounts.rs` unit tests (kinds,
  size, links, pipes, devices, folders, dangling and circular links) and
  the D-Bus test (`changes_a_users_name_picture_and_type`).
- Reading a user's picture path from AccountsService (shown in the lists)
  only accepts an absolute path with an image extension and no `..`; an odd
  one is shown as no picture.
- Fingerprints are fprintd's, claimed per enrolment and always released.

## 6. Opening links

`Qt.openUrlExternally` appears in a short, fixed list of places and the test
`qml_text.rs` fails when a new one is added:

| Where | URL | Rule |
|---|---|---|
| Updates, "Open ... Support" / "Search the Web" | built in Rust from the DMI maker (`MAKERS`) or a web search of it | `https` only, host in the maker table or `duckduckgo.com`, no user, port or odd characters (`support_url_allowed`); firmware text is only ever percent-encoded query data, cut at 120 bytes |
| Updates, release notes and history | links in the notes | `https` only (`telamon_updater_core::notes::is_safe_link`); the notes are sanitized there, and a link's visible text may not claim a different host |
| Crash Reports, "Report on GitHub Instead" / "View on GitHub" | built by the framework or stored with a sent report | `https://github.com/EternalCoder454/...` only: an issue number, or `<repo>/issues/new?title=...` percent-encoded (`crash_link_allowed`) |

The firmware support search sends the computer's maker and model to the search
engine, but only when the person presses the button for it.

## 7. Files Settings writes

- **Atomic**: KConfig writes a new file and renames it (`QSaveFile`); the
  Flatpak override files are written to a temporary file in the same folder
  (`O_EXCL`, so an existing link is never followed) and renamed over the old
  one; a half-written file is never seen.
- **Whose files**: only the user's own (`~/.config`, `~/.local/share/flatpak`),
  never system files (a system autostart entry is turned off by a user file
  with `Hidden=true`).
- **Links.** KConfig and `QFile::copy` write *through* a link, so a link put
  in a folder by something else would make Settings replace the file it points
  at. Autostart entries (the only files whose *names* come from outside) are
  written only when they are not links, or are links to a regular `.desktop`
  file (what a dotfiles manager makes); a link to anything else, or to nothing,
  is refused. Flatpak overrides are *replaced* by the new file when they are
  links (the link's target is left alone). Settings' other files have fixed
  names. *Residual:* a link at a fixed name (`kdeglobals`) that the user made
  themselves is followed on purpose, so dotfiles folders keep working.
- **Reading** files others may have made: Flatpak metadata and overrides
  are opened once, must be regular files and are capped at 256 KiB, so a pipe,
  a device or a file that grew after it was measured cannot stall Settings or
  fill its memory; Look-and-feel metadata is capped at 64 KiB; names and
  comments from desktop files are capped, and icon names must be theme names,
  not paths.
- **Modes**: new files get the umask's modes (KConfig) or `0644` (overrides,
  as Flatpak writes them); nothing Settings writes holds a secret.

## 8. Rich text

Everything from outside is drawn as **plain text**: every `Text` and
`QQC2.Label` in the app says `textFormat: Text.PlainText`, `TelamonLabel`
and Telamon.Ui's rows and banners are plain by default, and nothing asks for
`RichText`, `StyledText`, `MarkdownText` or `AutoText` (which would turn
`<img src="https://...">` in a user's full name into a request, and `<a href>`
into a link). The one rich text is the update release notes (`NotesText`), made
by the updater's sanitizer. *Test:* `qml_text.rs` (`drawn_text_is_plain`,
`nothing_asks_for_markup`, `only_the_release_notes_are_rich_text`,
`telamon_label_stays_plain`).

Untrusted D-Bus and file text is also cleaned in Rust: control and invisible
format characters (bidi overrides, zero-width marks) replaced, length capped
(`settings_sys::error::clean`).

## 9. Crash reports

The framework's code (`telamon-framework-system::crash`) owns the files, the
redaction and the sending; Settings' sheet only shows and asks.

- **Consent**: a report leaves the computer only when the person presses Send
  for that report; `crash::send` refuses when reporting is off or there is no
  server. "Don't Send" deletes it.
- **What is shown is what is sent**: the "Exact data" view is
  `Report::payload()`, the same function `send` posts.
- **Transport**: the framework posts with `/usr/bin/curl` from an argument
  vector: `https` only (plain `http` only to loopback), certificate checks on
  (curl's default; there is no `-k`), no config file, no proxy, no redirects,
  30 s and 64 KiB limits, and it keeps the relay's answer only if it is an
  issue of the project.
- GitHub fallback: a prefilled new-issue page the person submits themselves.

## 10. Updates and firmware

The Updates page is the Telamon Updater engine's (`telamon-updater-core`,
pinned by commit). Settings' part: it passes only values it was shown.

- **Firmware install** is pinned to what was on screen: the engine reads the
  release again and refuses if the version or checksum changed, or if fwupd
  says the release is not trusted, then downloads, checks and hands the file
  to fwupd. The person confirms first.
- **Fixture and test hooks** of the engine (`TELAMON_UPDATER_FIXTURES`...) are
  compiled out of release builds (`debug_assertions`/feature gated). Settings'
  own bus override, `TELAMON_SETTINGS_TEST_BUS`, is likewise debug only, and
  the fake-screens hook (`TELAMON_SETTINGS_FAKE_DISPLAYS`) is **not part of the
  program since 0.5.0**: it was compiled into the release binary; now only the
  test builds have it (`telamon-settings-displays-hooks`), and the package's
  `%check` fails if the string is in the program.

## 11. Build hardening and supply chain

- **RPM flags.** The spec builds with Fedora's `%build_cflags`,
  `%build_cxxflags`, `%build_ldflags` and `%build_rustflags` (stack protector
  strong, `_FORTIFY_SOURCE=3`, `_GLIBCXX_ASSERTIONS`, stack clash protection,
  `-fcf-protection`, PIE, full RELRO and `BIND_NOW`, `-Werror=format-security`,
  Rust's own hardening). The flags are *confirmed on the result*:
  `scripts/check-hardening.sh`, run by `%check` on both programs, reads the ELF
  files back and fails the package build without PIE, `GNU_RELRO` with
  `BIND_NOW`, a non-executable stack, no RPATH/RUNPATH/TEXTREL, and (for the
  C++ program) stack protectors and the absence of the tests' hooks. *Not
  met:* Intel CET's IBT marking. The C++ is built with `-fcf-protection`,
  but rustc has no stable switch for it and the linker marks a program only if
  every object in it is marked, so both programs carry the shadow stack mark
  (`SHSTK`) and not `IBT`; the check reports it as a note (`--require-cet`
  makes it an error). Fedora's `annocheck` accepts this for Rust programs, and
  userspace IBT is not enforced by Linux yet, so little is lost today.
- **Locked and pinned.** Builds use `--locked`; the two git dependencies
  (`atlas-framework` by tag, `atlasos-updater` by commit) are pinned and the
  framework tag is checked against CI; every GitHub Action is pinned by commit.
- **`cargo-deny`** (`deny.toml`: advisories, licences, sources, bans) and
  **`cargo-audit`** run in CI on every change and weekly, so a new advisory
  against the lock file is seen without a commit.
- **Property tests** (`proptest`) of the parsers that read what other programs
  send run in CI with far more cases than a local run does
  (`PROPTEST_CASES=20000`).
- Releases are tagged by hand and built in the Fedora 44 container; nothing is
  fetched at build time except crates and the two pinned git dependencies.

## 12. What is left

- **No sandbox for Settings itself.** It is a normal session app (it must
  reach the system bus, `~/.config` and the Plasma services). A Flatpak build
  is not planned.
- **Session `CommandLine` access.** A process that can talk to the session
  bus name can open any installed KCM. Fixing that needs the shell's bus
  filter (Flatpak's does) rather than another check here.
- **Secrets in Qt's memory**, as noted in 4.
- **The services and the pinned crates** (`atlas-framework`, `atlasos-updater`)
  are reviewed at their call sites, not inside; the firmware download, the
  crash sender and the release-notes sanitizer are theirs and should get the
  same audit in their repositories.
- **TOCTOU on a picture**: AccountsService opens the (resolved) path again
  after Settings checked it; its own privilege drop and checks are the
  defence there.
- **WEP and open Wi-Fi** can still be joined on purpose; the page warns.
- **Fuzzing is property testing** (stable Rust, no coverage guidance);
  `cargo-fuzz` targets are a possible next step.
