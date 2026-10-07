# Settings: design

What this file fixes: the layout, the threading rule, what is trusted, who
owns what, and the budgets. Change it together with the code that changes
them. The full plan and its reasons are the Atlas Notes note
"AtlasOS/Settings/Plan"; progress is in "AtlasOS/Settings/Roadmap".

## Scope

Settings replaces KDE System Settings on AtlasOS. Its pages do the work
themselves through the system's services (NetworkManager, BlueZ, UPower,
power-profiles-daemon, AccountsService, timedated, localed, hostnamed, the
portal PermissionStore, KDE's session services). A few rows stay KDE's
(Printers opens `kcm_printer_manager`), Updates opens Atlas Updater, and every
other installed KCM is listed under Other Plasma Settings and opens in
`kcmshell6`.

## Pages: simple first

Plasma's System Settings has around 30 top-level pages and 65 once opened,
grouped by how the system is built, with its internals' names and every
option in view: people can't find things. Windows 11 splits settings between
Settings and Control Panel and drops people into the old one; macOS has a
"General" junk drawer, an overfull Privacy page and long lists. Settings
keeps what they do well (Windows' fixed sidebar, values and toggles on the
rows, related links; macOS's search that shows where a setting lives) and
holds to these rules, which `crates/settings-registry/tests/pages.rs`
checks where it can:

- **Few pages, named for what people do**, in one flat sidebar with no
  headings: Home, Network, Bluetooth & Devices, Displays, Sound, Keyboard &
  Mouse, Appearance, Notifications, Apps, Privacy & Security, Users, Power &
  Battery, Accessibility, Time & Language, System. No junk drawer: System is
  Updates and About only.
- **The common settings first.** A page shows at most six settings, with
  good defaults; the rest of what it covers folds into one Advanced section
  at its end ([`Item::advanced`]). There is no global "advanced mode".
- **One level deep.** A page never opens a page; details (a network, a
  device, a user) open in a sheet over it.
- **Search finds everything**, folded settings and KCM rows too, shows where
  each lives ("Displays › Advanced"), opens the fold and points at the row.
- **No dead ends into Plasma for common settings.** A KCM is only a row for
  something few people change, and says that it opens Plasma's settings;
  every KCM without a row is under Other Plasma Settings, a link at the end
  of System that is not in the sidebar.
- **The state is on the row** (a toggle, the current value), and a page's
  related pages are links at its end.
- **A sidebar click always opens the page's top**, never a spot left from an
  earlier visit.
- **Home is short and has no promotions:** quick toggles and status (Wi-Fi,
  Bluetooth, Light or Dark, updates) and recently changed settings.
- Plain words, a different symbol for every page, and no Apply button.

Page IDs of earlier versions still open where their settings went
(`pages::RENAMED`).

Until cutover Settings installs beside System Settings, hidden from the menu
(`NoDisplay=true`), and replaces nothing of KDE's. At cutover a subpackage
takes over `systemsettings` (see "Entry points").

## Layout

- `crates/settings-registry`: no Qt, no dependencies. The page registry
  (`pages.rs`: pages, the settings on each page, which fold under
  Advanced, and the KCMs they answer for), KCM names and where they land (`kcm.rs`), search over pages
  and settings (`search.rs`), and launch arguments (`launch.rs`). The
  sidebar, search, deep links, Other Plasma Settings and the future `systemsettings`
  shim and KRunner runner all read it, so they can't drift apart.
- `crates/settings-sys`: no Qt. zbus 5 clients for the system's services,
  one module per service, with timeouts and plain-language errors
  (`error.rs`). Tested against python-dbusmock on a private bus.
- `apps/atlas-settings`: the CXX-Qt backend (`src/backend.rs`), `cpp/main.cpp`
  (Qt start, single instance, command line), `cpp/launcher.cpp` (starts
  `kcmshell6` and Atlas Updater), `cpp/kcmcatalog.cpp` (the installed KCMs,
  through KPluginMetaData) and `qml/`.
- Later: Displays (libkscreen) and the PipeWire side of Sound (libpulse) in
  C++, because their only stable API is C++/C; decided by spikes S1 and S2.

## Window

One `AtlasWindow`: an `AtlasSidebar` on the left, under a `SearchField`, and
the page beside it.

- **Sidebar:** the pages in one flat list, no headings ("Pages: simple
  first").
- **Search** lives in the content area, not the sidebar's filter: while the
  field has text, an `AtlasSearchResults` list replaces the page, with Up,
  Down and Enter handled from the field. Results are pages and single
  settings, ranked in Rust (`settings_registry::search`), at most 50.
- **Pages:** a native page is an `AtlasPage` of `Section`s and
  `SectionRow`s, its folded settings in a closed Advanced `Section` at the
  end. Until a page is built it shows "Coming Soon" with a button that opens
  the KCM it replaces. Every change applies at once; there is no Apply
  button.
- The last page shown is kept in `atlas-settingsrc` (`[Window] Page`); a new
  install starts on Home.

## Threads

The GUI thread never blocks on D-Bus or a child process. System calls run on
worker threads with a 10 s timeout (120 s for calls that may show a polkit
prompt) and post results back with `qt_thread().queue`. A page's backend and
its watchers live only while the page is shown.

## Launch arguments and single instance

`main.cpp` uses `KDBusService::Unique`. A second launch hands its arguments
to the first, which raises its window and passes them to `Backend.activate`.
Arguments are parsed in Rust (`settings_registry::launch`), never in C++ or
QML:

- `atlas-settings <page> [setting]`, `--page <page>`
- `--kcm <name> [--args <text>]`, `--kcm=<name>`
- `--search <text>`

At most 16 arguments of 256 bytes are read (`--args` up to 1 KiB). Control
characters, unknown options, unknown pages and settings, and anything that
isn't a KCM name are refused with a reason and shown in the window as plain
text. With nothing usable a launch shows the home page; a bare relaunch keeps
the page shown.

**KCM names** are `kcm`, an optional `_`, then a letter or digit and up to 64
of `[A-Za-z0-9_-]` (Plasma has `kcm_recentFiles` and `kcmspellchecking`). A
`.desktop` suffix and a `plasma/kcms/...` plugin path are accepted and
stripped; other paths are refused. A KCM Settings has a page for opens that
page; any other opens in `kcmshell6 <name> --args=<text>`, the arguments in
one `--args=` word so text starting with `-` can't be read as an option.
Programs start from argv vectors through `KIO::CommandLauncherJob` with the
window's activation token, never through a shell (`scripts/ui-stress.sh`
checks that shell syntax in `--args` runs nothing). They are found in
`/usr/bin` only, never through `PATH`. The same command twice within 2 s
starts once, and at most 5 programs start in any 10 s, since other programs'
links can ask for them.

## Entry points (at cutover)

Today `systemsettings <kcm>` (KCMLauncher, the tray applets, the `kcm_*`
launchers, the image's scripts) opens KDE's System Settings. At cutover a
subpackage `atlas-settings-systemsettings` `Obsoletes:` and `Provides:`
plasma-systemsettings and installs:

1. `/usr/bin/systemsettings`, a small Rust binary that validates its argv
   with the same registry code and execs `atlas-settings --kcm <name>` for a
   KCM Settings has a page for, else `kcmshell6 <name>`. No shell, no PATH
   lookup beyond the two fixed binaries.
2. Hidden `systemsettings.desktop` and `kdesystemsettings.desktop` that
   launch Settings, so KCMLauncher keeps finding System Settings and old pins
   keep working.

The AtlasOS Launcher, which replaces KRunner, finds Settings pages through
`/usr/share/atlas-settings/search-index.json`, generated from the registry,
and opens them with `org.freedesktop.Application.ActivateAction` (`open`,
`open-app`). This is planned for F1; the format is in the Plan note.

The image's own changes (menu entry, dock pin, scripts) are the AtlasOS
session's. Direct `kcmshell6` calls (Dolphin's trash, KNotifications) keep
opening that one KDE page.

`crates/settings-registry/tests/kcm_names.rs` checks every KCM name the
applets and the image use against a fixture of Plasma 6.7's KCMs.

## Trust

- **Launch arguments** come from any process in the session: validated as
  above.
- **D-Bus replies** (SSIDs, device, printer and user names) are untrusted:
  shown as plain text only, length-capped, control characters removed, SSIDs
  decoded lossily.
- **KCM metadata** from installed plugins is shown as plain text, names
  capped at 200 characters and descriptions at 400; icons must be theme
  names, not paths. KCM names from it pass the same name check before they
  reach `kcmshell6`, and plugins whose ID isn't a KCM name aren't listed.
- **Service errors** keep at most 200 characters of the service's message,
  control and invisible format characters replaced; values read back
  (a time zone) pass the same check as values sent.
- **Privileges:** none of our own. Every privileged change goes through the
  owning service's polkit action. No helper, no setuid, no new polkit action.
- **Secrets:** passwords and PINs are never logged and are held only for the
  call. Wi-Fi passwords go to NetworkManager and are never stored by us.
- **Files:** only the user's own, written atomically (KConfig or
  AtlasSettings).

## Strings and translation

No sibling Atlas app ships translations yet. QML strings use `qsTr()`; the
registry's page titles, setting titles and keywords are English in Rust.
When translation comes, the registry's strings get IDs that QML translates,
and search matches the translated text too.

## Budgets

| What | Budget |
|---|---|
| Launch to first frame | ≤ 250 ms |
| First visit of a page to content | ≤ 150 ms; a visited page ≤ 50 ms |
| Search, per keystroke | ≤ 16 ms |
| Idle CPU, window shown or hidden | 0 % over 60 s |
| RSS after launch | ≤ 140 MB |
| RSS after visiting every page | ≤ 220 MB, and below System Settings |
| `systemsettings` shim, exec to hand-off | ≤ 5 ms |
