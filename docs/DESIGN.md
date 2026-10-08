# Settings: design

What this file fixes: the layout, the threading rule, what is trusted, who
owns what, and the budgets. Change it together with the code that changes
them. The full plan and its reasons are the Atlas Notes note
"AtlasOS/Settings/Plan"; progress is in "AtlasOS/Settings/Roadmap".

## Scope

Settings replaces KDE System Settings on Telamon OS. Its pages do the work
themselves through the system's services (NetworkManager, BlueZ, UPower,
power-profiles-daemon, AccountsService, timedated, localed, hostnamed, the
portal PermissionStore, KDE's session services, the system helper for
updates). A few rows stay KDE's (Printers opens `kcm_printer_manager`), and
every other installed KCM is listed under Other Plasma Settings and opens in
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
  Mouse, Appearance, Notifications, Apps, Privacy & Security, Accounts, Power &
  Battery, Accessibility, Time & Language, System, Updates. No junk drawer:
  System is About only, and Updates is its own page, as in Windows, last in
  the list.
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
  Bluetooth, Light or Dark) and recently changed settings.
- Plain words, a different symbol for every page, and no Apply button.

Page IDs of earlier versions still open where their settings went
(`pages::RENAMED`).

Settings is in the menu and the Launcher, and since the 0.4.0 cutover a
subpackage takes over `systemsettings` (see "Entry points").

## Layout

- `crates/settings-registry`: no Qt, no dependencies. The page registry
  (`pages.rs`: pages, the settings on each page, which fold under
  Advanced, and the KCMs they answer for), KCM names and where they land (`kcm.rs`), search over pages
  and settings (`search.rs`), and launch arguments (`launch.rs`). The
  sidebar, search, deep links, Other Plasma Settings, the `systemsettings`
  shim and the Launcher's search index all read it, so they can't drift
  apart. `index.rs` writes that index (`src/bin/gen-search-index.rs`, run by
  the package build) and `launch.rs` also reads the `ActivateAction` calls
  the Launcher makes with it.
- `crates/systemsettings-shim`: no Qt. `/usr/bin/systemsettings` (see "Entry
  points"): `plan()` decides, `main.rs` replaces the process.
- `crates/settings-sys`: no Qt. zbus 5 clients for the system's services,
  one module per service, with timeouts and plain-language errors
  (`error.rs`). Tested against python-dbusmock on a private bus.
- `apps/telamon-settings`: the CXX-Qt backend (`src/backend.rs`), one
  backend QObject per built page (`src/time_language.rs`,
  `src/system_info.rs`) and the worker-thread helper (`src/worker.rs`),
  the Updates page's backend (`src/updates_page.rs`, over
  telamon-updater-core, see "Updates"),
  `cpp/main.cpp` (Qt start, single instance, command line),
  `cpp/launcher.cpp` (starts `kcmshell6`),
  `cpp/kcmcatalog.cpp` (the installed KCMs, through KPluginMetaData),
  `cpp/pagebackends.cpp` (makes a page's backends when it is shown),
  `cpp/localeconfig.cpp` (Plasma's `plasma-localerc`, through KConfig), the
  KConfig and KWin side of Appearance, Keyboard & Mouse, Notifications and
  Accessibility (`cpp/appearanceconfig.cpp` and the like, see below) and
  `qml/`.

  `cpp/localeconfig.cpp` (Plasma's `plasma-localerc`, through KConfig),
  `cpp/powerconfig.cpp`, `cpp/screenlockconfig.cpp`,
  `cpp/autostartconfig.cpp` and `cpp/defaultapps.cpp` (more KConfig and
  KService) and `qml/`.
- The page kit in `qml/`: `SettingsPage` (an `TelamonPage` with the page's
  registry entry; scrolls to and briefly highlights the row a link or
  search asked for, found by its `objectName` = the item ID, and opens
  Advanced first when the item is folded), `AdvancedSection` (the closed
  Advanced `Section`, which adds a `KcmRow` for each folded KCM item of the
  registry), `KcmRow` (says it opens Plasma's settings), `RelatedLinks`
  (the registry's `related` pages) and `PickerSheet` (a searchable list in
  a sheet). `Main.qml` maps page IDs to their components (`nativePages`);
  any other page shows `PendingPage`.
- Displays (libkscreen) and the PipeWire side of Sound (PulseAudioQt) are in
  C++, because their only stable API is C++/C (see "Displays and Sound").

## Window

One `TelamonWindow`: an `TelamonSidebar` on the left, under a `SearchField`, and
the page beside it.

- **Sidebar:** the pages in one flat list, no headings ("Pages: simple
  first").
- **Search** lives in the content area, not the sidebar's filter: while the
  field has text, an `TelamonSearchResults` list replaces the page, with Up,
  Down and Enter handled from the field. Results are pages and single
  settings, ranked in Rust (`settings_registry::search`), at most 50.
- **Pages:** a native page is a `SettingsPage` of `Section`s and
  `SectionRow`s, its folded settings in a closed `AdvancedSection` at the
  end, then its `RelatedLinks`. A sidebar click or link to the page shown
  makes the page anew, so it opens at its top with Advanced closed. Until a page is built it shows "Coming Soon" with a button that opens
  the KCM it replaces. Every change applies at once; there is no Apply
  button.
- The last page shown is kept in `telamon-settingsrc` (`[Window] Page`); a new
  install starts on Home.

## Threads

The GUI thread never blocks on D-Bus or a child process. System calls run on
worker threads with a 10 s timeout (120 s for calls that may show a polkit
prompt) and post results back with `qt_thread().queue` (`src/worker.rs`). A
page's backend and its watchers live only while the page is shown: the
page asks `PageBackends.create(kind, page)` for them, which parents them to
the page, and a result that comes back after the page is gone is dropped.

## Time & Language and System

- **Time:** network time and the time zone are timedated's (`SetNTP`,
  `SetTimezone`, the list from `ListTimezones`).
- **The user's language and formats** are Plasma's, as Plasma 6.7's
  Region & Language KCM keeps them and startplasma reads them at sign-in:
  `plasma-localerc` `[Formats]` `LANG` and `LC_*`, `[Translations]`
  `LANGUAGE`, written through KConfig (atomically). Language sets `LANG`
  and `LANGUAGE`; Formats sets every `LC_*` the KCM writes, or removes them
  to follow the language. Plasma has no 24-hour switch of its own (the
  clock follows `LC_TIME`), so 24-Hour Time sets `LC_TIME` to a locale of
  the same language that writes times the asked way (`en_GB` for `en_US`),
  or removes it when the formats already do. Changes apply at the next
  sign-in, which the page says. Unlike the KCM, Settings doesn't yet tell
  AccountsService (`SetLanguages`) or generate missing locales.
- **The login screen's language** is localed's `LANG` (`SetLocale`).
- **System:** the device name is hostnamed's pretty hostname; the static
  hostname is derived from it (lower-case ASCII letters, digits and
  hyphens), and kept when nothing of the name is left. The version, the
  processor, the memory and the graphics are read from `/usr/lib/os-release`,
  `/proc` and `/sys` and the PCI ID database, capped and made safe to show
  (`settings_sys::sysinfo`); nothing privileged.

## Network, Bluetooth & Devices and Home

- **Network** is NetworkManager's, through `settings_sys::network`: Wi-Fi on
  or off (`WirelessEnabled`), the networks in range (one row per name, the
  strongest access point; the one in use first, then the saved ones), wired
  status, VPN connections (`vpn` and `wireguard` profiles, connected and
  disconnected from the row), the hotspot and airplane mode. A network opens
  in a sheet (status, signal, security; Join with a password, Connect,
  Disconnect, Forget); a new password goes to NetworkManager in the
  `AddAndActivateConnection` call and is never stored or logged
  (`network::Secret` hides it from `Debug` and overwrites it when dropped). A
  network that doesn't come up within 30 s, or is refused, is not kept. WPA,
  WPA3 and WEP passwords are checked before they are sent; networks that need
  a login (802.1X) are set up in Plasma's editor (`kcm_networkmanagement`,
  which a sheet and "Add a VPN" open). The hotspot is one saved profile
  (`mode=ap`, `ipv4.method=shared`, WPA2): the switch starts the saved one, or
  asks for a name and a made-up password the first time; "Change…" replaces
  it. Airplane mode switches Wi-Fi and mobile broadband (NetworkManager) and
  Bluetooth (BlueZ) off together. SSIDs and connection names are decoded
  lossily, capped at 32 and 64 characters and cleaned like other D-Bus text;
  hidden networks (no name) aren't listed.
- **Bluetooth & Devices** is BlueZ's, through `settings_sys::bluetooth`: one
  `GetManagedObjects` read gives the adapter and its devices, which the page
  names by address (the client looks the object path up itself, so a page
  never sends a path). Paired devices are always listed; an unpaired one only
  while it is in range and names itself. A click pairs, trusts and connects a
  nearby device; a paired one opens a sheet (connect, disconnect, forget,
  battery from `Battery1`). Pairing that needs a PIN or a confirmation is
  asked by the session's Bluetooth agent (Plasma's BlueDevil): Settings
  registers none of its own yet. Looking for devices runs on a thread of its
  own that holds the D-Bus connection (BlueZ ends a client's search when its
  connection goes), only while the page is shown, Bluetooth is on and the
  window isn't minimized, and ends when the page goes. Visible to Other
  Devices (Advanced) is the adapter's `Discoverable`; BlueZ turns it off after
  its own timeout. Printers opens `kcm_printer_manager`.
- **Both pages look again while shown** (every 6 s and 3 s, not while the
  window is minimized) instead of subscribing to D-Bus signals, so a page has
  no watcher thread to end; each read runs on a worker thread and is skipped
  while another is under way or a change is in progress.
- **Home** is the device name and the system's name, three switches (Wi-Fi
  and Bluetooth, which use the Network and Bluetooth backends; Dark Mode) and
  links to six pages (the registry's `related` for `home`). Dark Mode reads
  `[General] ColorScheme` of `kdeglobals` through KConfig
  (`cpp/colorscheme.cpp`) and switches between the image's `AtlasOSLight` and
  `AtlasOSDark` by starting `plasma-apply-colorscheme <name>` through the
  Launcher (argv, `/usr/bin` only); Plasma writes the file and repaints. The
  Appearance page does the same with the same two names.
- **Smoke runs with services:** a debug build started with
  `TELAMON_SETTINGS_TEST_BUS=<address>` uses that bus instead of the system bus
  (`src/support.rs`; release builds ignore it).
  `scripts/with-mock-services.py <command>` starts a private bus with
  python-dbusmock's NetworkManager and BlueZ, filled with a few networks and
  devices, and runs the command with the variable set:
  `scripts/dev.sh scripts/with-mock-services.py scripts/smoke.sh network`.

## Displays and Sound

Both are QML pages over a small C++ QObject made by `PageBackends` when the
page is shown and gone with it; the logic that needs no screen or sound
server is in plain Qt files that Qt Test covers (`apps/telamon-settings/tests`,
run with `ctest`).

- **Displays** reads and sets the screens through libkscreen
  (`cpp/screenconfig.cpp`: `GetConfigOperation`, `SetConfigOperation`; KWin's
  output management on Wayland), as Plasma 6.7's Display Configuration does:
  scale from 50 % to 300 % in 5 % steps (every step is exact in 1/120, the
  Wayland fractional-scale unit), the mode kept at the same refresh rate when
  the new resolution has it and the fastest otherwise, rotation as its four
  turns, HDR and wide colour gamut together, the main screen as priority 1.
  The screens must touch along an edge and never overlap; a drag snaps to the
  nearest edge (`cpp/displaylogic.cpp`) and positions start at 0, 0.
  Resolution, scale, rate, rotation, HDR and arrangement ask "Keep these
  settings?" for 15 s and put the earlier settings back when nobody answers,
  or when Settings closes with the question open; making a screen the main
  one doesn't ask. Scale is applied when the slider is let go. Night Light is
  `kwinrc [NightColor]` (`Active`, `Mode` 0 or 1, `NightTemperature`) through
  KConfig, which KWin watches, with `org.kde.KWin.NightLight` asked
  asynchronously for whether it works and to preview a temperature while the
  slider moves (`cpp/nightlight.cpp`). The schedule is Plasma's "Sunrise and
  sunset" (Dark-Light schedule, set in Plasma's Night Time page) or all day.
  A screen's name is its vendor and model (the connector's name when it has
  none), shown as plain text.
- **Sound** talks to the PulseAudio API on PipeWire through PulseAudioQt
  (`cpp/soundmixer.cpp`), which Plasma uses too, on the GUI thread's event
  loop. Nothing is applied: every slider is live and the server tells the
  page about changes from anywhere. The lists leave out virtual devices (but
  not the default one), monitors of outputs, streams that are paused or
  virtual, and the system's own event sounds. App names and icons come from
  the stream's properties, made safe to show (a theme icon name, never a
  path).
- **Testing without screens or sound.** `TELAMON_SETTINGS_FAKE_DISPLAYS=<json>`
  (built with `TELAMON_SETTINGS_TEST_HOOKS`, on by default) gives Displays
  libkscreen's Fake backend with the layout in that file
  (`tests/fixtures/displays-two.json`: a 3840x2160 screen at 1.7x with an
  upright 1920x1080 one); it is the only place Settings uses a private
  libkscreen header. The Sound test (`soundmixer_test`) needs a private
  PipeWire with null devices and skips without one
  (`TELAMON_TEST_SOUND=private`). Neither touches the real screens or audio.

## Appearance, Keyboard & Mouse, Notifications and Accessibility

These pages write Plasma's own settings the way Plasma's modules do: the same
files, groups and keys, through KConfig with `KConfig::Notify` (so Plasma,
KWin and apps that watch the file pick the change up live), or the same
service call. Their backends are C++ QObjects (`cpp/appearanceconfig.*`,
`inputconfig.*`, `notificationsconfig.*`, `accessibilityconfig.*`, helpers in
`kdeutil.h`): KConfig has no Rust binding here, and the D-Bus calls are
asynchronous QtDBus calls, so the GUI thread still never waits. Programs go
to the page through a `run(argv)` signal and then through the Launcher; no
backend starts one itself. Tests (`tests/pageconfig_test.cpp`, `ctest`) read
every write back from a temporary `$XDG_CONFIG_HOME` and run the KWin and
Plasma shell calls against fakes (`tests/fakes.h`) on a private session bus.

- **The desktop preview** tops the page: a 16:10 picture of the Telamon OS
  desktop (`qml/DesktopPreview.qml`) that follows Light or Dark, the accent,
  the wallpaper, transparency and the dock, and previews a Light or Dark card
  or an accent swatch while the pointer or the keyboard focus is on it (a note
  under it says so). It is never a control, and shows nothing of the user's
  but the wallpaper: the window is made up, the dock holds generic theme icons
  and the bar's islands are bare shapes. The colours are the ones TelamonStyle
  derives from the TelamonLight and TelamonDark schemes, whichever scheme the
  app is in. Translucent means `Appearance.effective` (the switch on and the
  compositor's blur available): the window, bar and dock then show a blurred
  copy of the wallpaper (a `MultiEffect`; none under software rendering, where
  they are only tinted). The wallpaper is `read()`'s `wallpaper.picture` (and
  `pictureDark`, which Plasma shows with a dark scheme): the main screen's
  (`lastScreen=0`) image of the applets file, a package's largest image up to
  2560 wide, Telamon OS's own wallpaper when none is set (and, until the image
  has loaded, the cherry tree's colours drawn as shapes); it is decoded at the
  size shown. A wallpaper just chosen shows at once and until Plasma has saved
  it (it does so some seconds later; `AppearanceConfig` watches the applets
  file). Colours fade over `TelamonStyle.durationShort`, which is 0 under
  reduced motion. `DesktopPreview` started as Telamon Setup's
  `WizardThemePreview` and is a candidate for Telamon.Ui (with the wallpaper,
  dock and blur as options): ask the framework session before a second app
  copies it.
- **Light or Dark** runs `plasma-apply-colorscheme TelamonLight|TelamonDark`,
  which writes `[General] ColorScheme` into the user's kdeglobals and
  announces it, and then, when an accent is set, `plasma-apply-colorscheme
  --accent-color` once the name has landed (with `--accent-color` the tool
  ignores a scheme named after it and only re-tints the current one, so the
  two are separate calls). When the scheme is current already by the Global
  Theme's defaults (`kdedefaults`) alone, the tool says "already set" and
  writes nothing: Settings writes the name itself, with a notification.
  Announced are the keys
  kvantum-sync watches, and gtkconfig, which follows for GTK. The icons
  (`Papirus`/`Papirus-Dark`) and the Aurorae window decoration follow when
  they are Telamon OS's own; a theme the user picked stays. The scheme shown is
  read as kvantum-sync reads it (kdeglobals, `kdedefaults/kdeglobals`, then
  /etc/xdg).
- **Accent** is `plasma-apply-colorscheme --accent-color` (kdeglobals
  `AccentColor`), or `accentColorFromWallpaper=true`, which Plasma's accent
  service follows. Violet, the default, is the scheme's own accent.
- **High Contrast** (Accessibility) makes `AtlasOSHighContrast{Light,Dark}`
  from the Telamon OS scheme in `~/.local/share/color-schemes` and applies it;
  Light or Dark then switches between the two high contrast schemes.
  Plasma has no switch of its own. Kvantum's themes are fixed colours, so
  Qt widgets of apps that use Kvantum keep their look; palette-based apps
  (Atlas apps, Plasma, GTK through gtkconfig) follow.
- **Wallpaper** is `plasma-apply-wallpaperimage <package or file>`, only
  for what the page lists (`/usr/share/wallpapers`, `~/.local/share/
  wallpapers` and `~/Pictures`). The current one is read from the applets
  file, read only.
- **Transparency** is Telamon.Ui's `Appearance.transparency` (`atlasrc`
  `[Appearance] Transparency`), the key kvantum-sync and every Atlas app
  watch.
- **Dock and top bar** are Plasma panels, read and changed with Plasma's panel
  scripting (`org.kde.PlasmaShell.evaluateScript`), as the Telamon OS menu bar
  toggle does: the dock is the panel with the task manager and the Telamon OS
  dock separator. Scripts are fixed text with validated values only.
- **Virtual Desktops** are KWin's `org.kde.KWin.VirtualDesktopManager`
  (`createDesktop`, `removeDesktop`); **Hot Corners** are kwinrc
  `[ElectricBorders]` and `[Effect-overview] BorderActivate`;
  **Global Theme** is `plasma-apply-lookandfeel --apply`, for the themes
  listed.
- **Input sources** are kxkbrc `[Layout]` (`Use`, `LayoutList`,
  `VariantList`, `DisplayNames`) as kcm_keyboard saves them, up to four, from
  xkeyboard-config's `evdev.xml`. **Key repeat and Num Lock** are kcminputrc
  `[Keyboard]` (`RepeatDelay`, `RepeatRate`, `NumLock`).
- **Pointers and touchpads** are KWin's input devices
  (`org.kde.KWin.InputDevice`): the settings are properties KWin applies at
  once and keeps in kcminputrc itself, as kcm_mouse and kcm_touchpad do.
  Speed, scrolling and the primary button are set on every device that
  supports them; tapping only on touchpads; the rows show when KWin lists a
  pointer (a touchpad row only with a touchpad).
- **Do Not Disturb** is plasmanotifyrc `[DoNotDisturb] Until`; **apps** are
  `[Applications][<desktop entry>]` `ShowPopups`, `ShowInHistory` and
  `ShowBadges`; **popups** are `[Notifications] PopupPosition`,
  `PopupTimeout` and `ShowPopupTimeout`. Plasma 6.7 has no setting for the
  lock screen's notifications that Settings could find, so that row opens
  Plasma's Screen Locking module.
- **Text Size** scales the six fonts of kdeglobals from the sizes the system
  sets (what "Adjust All Fonts" does) and sends `refreshFonts` to the
  platform theme. **Reduce Motion** is `[KDE] AnimationDurationFactor` = 0
  (removed again when turned off, which brings back the system's speed).
  **Screen Reader** is kaccessrc `[ScreenReader] Enabled` (kaccess starts
  Orca) and the `screen-reader-enabled` GSettings key, as kcm_access sets
  them. **Zoom** is kwinrc `[Plugins] zoomEnabled` and the effect;
  **Sticky Keys** and the other helps are kaccessrc `[Keyboard]` and
  `[Mouse]`.

## Power & Battery, Accounts, Privacy & Security and Apps

Each page's system calls are `settings-sys` clients tested against
python-dbusmock (templates for the services it has none for are in
`crates/settings-sys/tests/templates`); each page's files are KConfig or
KService on the C++ side, tested against a temporary `XDG_CONFIG_HOME`
(`apps/telamon-settings/tests/configtests.cpp`, `-DTELAMON_SETTINGS_TESTS=ON`).

- **Power & Battery.** The power mode is power-profiles-daemon's
  `ActiveProfile` (`net.hadess.PowerProfiles`, or UPower's name for it from
  0.20). The battery is the first UPower device that is a present power
  supply battery; a computer without one shows no battery rows (and no lid
  row without `LidIsPresent`). The charge limit is UPower's
  `EnableChargeThreshold`, shown only when `ChargeThresholdSupported`.
  Turn Off the Screen, Sleep, the lid and the power button are
  `powerdevilrc` as kcm_powerdevilprofilesconfig writes it
  (`[AC]`/`[Battery]` `[Display]` and `[SuspendAndShutdown]`; "never" is the
  idle switch off and `-1`), written through KConfig with the change
  notification, then PowerDevil's `reparseConfiguration`. A computer with
  a battery writes both profiles. Defaults for keys the file doesn't have
  are PowerDevil's (`cpp/powerconfig.cpp`).
- **Accounts** (page ID `users`, so links and searches for Users still land
  on it). It reads as your account first: a header with your picture (tap it
  to change it, or the camera on it to take one: `Launcher::runApplication`
  starts Plasma Camera, else Kamoso, Snapshot or Cheese, from its desktop file
  through KIO, and a banner offers the file chooser at Pictures, where Camera
  saves; with none installed it opens Telamon Store at Snapshot), full name, "user name · Administrator" and the buttons
  Change Picture and Edit Name; then Sign-In Options (Password, Fingerprint
  when there is a reader, Automatic Login); then Other Users (each person
  opens a sheet to make them an administrator or remove them, and the last
  row is Add User; with nobody else it says so). Advanced holds only the
  login screen's KCM. AccountsService for the users, with the signed-in user
  found by uid. Pictures are round: `TelamonAvatar` masks them with a GPU
  effect, which this app's default CPU drawing cannot do (it shows them
  square), so `qml/AccountAvatar.qml` crops them to a circle on a Canvas and
  uses `TelamonAvatar` only for the initials. Framework gap: drop it when
  `TelamonAvatar` rounds pictures on the software backend. `SetPassword`
  takes a SHA-512 crypt(3) hash, made with libcrypt and a random salt (`settings_sys::accounts::hash_password`): the clear
  text is held for the call only (overwritten after) and is never logged.
  Calls that may ask for a password send the polkit interactive flag.
  Add User creates the user and sets the password, and removes the user
  again when the password can't be set. Fingerprints are fprintd's:
  enrolling claims the reader, listens for `EnrollStatus` and always calls
  `EnrollStop` and `Release`.
- **Privacy & Security.** Screen Lock is `kscreenlockerrc` `[Daemon]`
  (`Autolock`, `Timeout`, `LockOnResume`). Location Services is GeoClue's
  agent (`geoclue-demo-agent.desktop`, which Plasma has no agent of its
  own for): turned off by a `Hidden=true` file of the same name in
  `~/.config/autostart`, so it takes effect at the next sign-in. The
  Firewall switch starts and enables, or stops and disables,
  `firewalld.service` through systemd; firewalld is only asked while
  systemd says it runs, because a call would start it again. Allowed Apps
  and Ports are the runtime zone API for the zone in use, made permanent
  with `runtimeToPermanent`. Crash Reports are telamon-framework-system's
  per-user setting (`~/.config/telamon/crash-reporting.toml`; the old
  `~/.config/atlas/` file is read until the new one exists), saved with the
  same call Updater uses, then Telamon Updater's tray is told to `Reload`
  (it collects the reports and says when one waits). "Review Crash Reports"
  (a sheet, `src/crash_reports.rs`, the Crash Reports screens of Updater's old
  window) lists the reports waiting, each a closed row that opens to the
  data that would be sent (the traces are as tall as their text: a view that
  scrolled by itself would take the wheel from the sheet), and sends one only
  when the person presses Send for it (the framework's `crash::send`); Don't
  Send deletes it; the reports sent in the last 90 days are listed below.
  Links come from the sheet's data only when `https://`. Which crashes are
  collected (the host's own, not containers' or programs outside the OS) is
  the framework's rule since 2.0.2. A `TelamonDialog` scrolls its body in a
  bare Flickable, which moves 28 to 38 px a wheel notch, unevenly; the
  dialogs whose lists can outgrow the window hold a `DialogScroll`, which
  gives it the pages' `Kirigami.WheelHandler` (60 px a notch).
- **Firmware on the Updates page** has the computer's own firmware (BIOS or
  UEFI) as its first row: vendor, version and date from the kernel's
  `/sys/class/dmi/id/bios_*` (`src/system_firmware.rs`; untrusted text,
  cleaned and capped; the date is SMBIOS `mm/dd/yyyy`). fwupd's list says what
  can be updated, not what is up to date: after a check the backend asks
  fwupd for the releases of its "System Firmware" device (`GetReleases`), and
  "Firmware is up to date." shows only when there are releases and none is
  newer. With none at all (`NothingToDo`: the maker doesn't publish to the
  LVFS) the page says so and opens the maker's support site (`MAKERS` in that
  file, from the DMI maker; else a web search for it); when fwupd can't say,
  "No firmware updates were found.".
- **Apps.** Default Apps are `~/.config/mimeapps.list` (`[Default
  Applications]`, `[Added Associations]`) for the file types a kind covers,
  chosen only among the apps KService offers for it; the terminal is
  `kdeglobals` `[General]` `TerminalApplication` and `TerminalService`.
  Startup Apps are the XDG autostart folders: a system entry is turned off by
  a user file with `Hidden=true`, never by editing the system's file.
  App Permissions are, for each installed Flatpak app, the user's override
  file (`~/.local/share/flatpak/overrides/<app>`, `[Context]`; only what
  differs from the app's metadata and the global override is written, other
  groups and keys are kept) and the portals' PermissionStore for
  background, camera, microphone and screen access.

## Updates

Updates is the page Telamon Updater's window was (Windows Update living in
Settings, not beside it). The window is gone from the Updater repo; what is
left there runs in the background: the tray (panel icon, schedule,
notifications, background app rounds), the system helper and the screen
glow. Settings uses the same code, not a copy of it: `telamon-updater-core`
(the repository `atlasos-updater`, a git dependency pinned to a commit in
`Cargo.toml`) holds the Qt-free logic (the system helper's client and
progress parser from `telamon-update-engine`, the settings file, schedule,
restart and locks from `telamon-updater-base`, Flatpak updates, firmware,
release notes, history), and `src/updates_page.rs` is the QObject around it,
the old window's backend without its crash report screens.

- **The page**: the status on top (Telamon OS is up to date, an update is
  available, downloading and installing with a bar, restart to finish) with
  the one next step (Check for Updates, Download Update, Restart to Update,
  Restart Tonight or at a time, Try Again); What's New (the release notes,
  in a sheet); App Updates (Flatpak, with "Update Apps"); Firmware Updates
  (only with fwupd); under Advanced Go Back to the Previous Version, Update
  Channel (stable or testing), Update Apps in the Background, and Update
  History (the versions this computer ran with their release notes, and the
  app updates), each in a sheet. A link to Privacy & Security is where
  crash reports are.
- **The system helper** is the Updater package's (`telamon-system-helper`,
  D-Bus name `net.eterneon.telamon.SystemHelper`, six methods and a
  `Progress` property, polkit actions of its own). Settings adds no method,
  helper or polkit action; every call asks polkit through the helper, which
  keeps "no root helper of our own" true for Settings.
- **The backend lives as long as the window**, not as long as the page
  (`Main.qml` makes it on the first visit): an update, app update or firmware
  install that runs while another page is shown goes on and shows again when
  the page does. Closing the window while the system is being changed
  (`working`) only hides it; the program goes when that ends
  (`cpp/main.cpp`, `keepRunning`). The first read of the page asks the
  helper (D-Bus activated), so a visit to another page never starts it.
- **The glow** ("the OS image is being changed") belongs to Telamon Updater's
  tray, which starts `telamon-updater-glow`, the one program that draws it
  around the edges of every screen, **only while the OS image is being
  changed**: an update being downloaded and staged (`busyOp` `download`), a
  channel switch, a go back or cancelling one (`switch`, `rollback`,
  `cancelRollback`: `glow_wanted` in `updates_page.rs`, unit tested). Not for
  app (Flatpak) updates, firmware installs or checks: those keep the window
  from quitting (`working`) but never glow. Settings tells the tray when that
  starts and ends (`settings_sys::updater::set_working`, the tray's
  `SetWorking` on the session bus; the tray forgets it when Settings leaves
  the bus, so a crash leaves no glow). An update, switch or go back that
  outlives Settings is seen by the tray itself, from the helper's `Progress`
  property.
- **Links**: `telamon-settings updates` opens the page and `updates check`
  opens it and starts a check (the tray's Check for Updates, the
  notifications' actions); `kcm_updates` is the page; the System page's
  related links lead to it.
- **Fixtures**: `TELAMON_UPDATER_FIXTURES=<dir>` (a debug or `fixtures`
  build; the states are in the Updater repo, `crates/telamon-updater-core/fixtures-states`
  and are copied to `apps/telamon-settings/fixtures-states` for the smoke
  runs) shows the page with a banner "Developer test data" and
  never touches the helper, Flatpak, fwupd or the user's settings file.
  `scripts/smoke-mock.sh updates` instead starts a mock helper
  (python-dbusmock, `crates/settings-sys/tests/templates/system_helper.py`)
  on the private bus.

## Launch arguments and single instance

`main.cpp` uses `KDBusService::Unique`. A second launch hands its arguments
to the first, which raises its window and passes them to `Backend.activate`.
Arguments are parsed in Rust (`settings_registry::launch`), never in C++ or
QML:

- `telamon-settings <page> [setting]`, `--page <page>`
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

## Entry points

`systemsettings <kcm>` (Plasma's KCMLauncher, the tray applets, the image's
scripts) used to open KDE's System Settings. Since the 0.4.0 cutover the
subpackage `telamon-settings-systemsettings` takes it over: it `Obsoletes:`
plasma-systemsettings (every version below 100) and `Provides:` its name, with
and without the architecture (plasma-desktop, colord-kde and kcm-plasmalogin
require it), at version 99 so no versioned `Requires:` from KDE can ask more.
Both packages own `/usr/bin/systemsettings`, so the old one has to go, and
the `Obsoletes:` does that in the same transaction. The subpackage installs:

1. `/usr/bin/systemsettings`, a small Rust binary (`crates/systemsettings-shim`)
   that validates its argv with the registry's `launch::parse` and execs
   `/usr/bin/telamon-settings --kcm <name>` for a KCM Settings has a page for
   (no arguments for System Settings' own start page), else
   `/usr/bin/kcmshell6 <name> [--args=<text>]`. Both are absolute paths: no
   shell, no `PATH` lookup. It accepts what KDE's `systemsettings` does with
   a module (`kcm_x`, `kcm_x.desktop`, a plugin path, `--args <text>`, `--help`,
   `--version`) and refuses anything else with status 2 and one line on the
   standard error, rather than guess. The text for a page's `--args` is not
   passed on (a page has no use for it). Exec to hand-off is within 5 ms
   (about 0.4 ms measured in a debug build, `tests/process.rs`).
2. Hidden `systemsettings.desktop` and `kdesystemsettings.desktop` that
   launch Settings, so KCMLauncher keeps finding System Settings and old pins
   keep working. They carry no `StartupWMClass`: the window's app id is
   `net.eterneon.telamon.settings`, the real desktop file's own name, and a
   second file claiming it as its window class makes Plasma's task manager
   match the window to the hidden one instead of the dock pin, so Settings
   showed up as a second icon. (An old pin on one of the hidden files
   starts Settings but no longer groups with its window.)

System Settings' global shortcut, Meta+I (and the Tools key), moves to
Settings: its desktop file carries `X-KDE-Shortcuts` and a copy is installed
in `kglobalaccel/`, where Plasma reads default shortcuts.

What goes with plasma-systemsettings: its KRunner plugin (the Launcher
replaces KRunner), its category files (Other Plasma Settings groups by the
KCMs' own metadata) and its zsh completion.

The Telamon OS Launcher, which replaces KRunner, finds Settings pages through
`/usr/share/telamon-settings/search-index.json` (format version 1: one entry
per page and per setting, `link` = `page` or `page/item`, text in locale maps
with a `C` fallback; `index.rs`, written at build time from the registry), and
opens them with `org.freedesktop.Application.ActivateAction` on
`net.eterneon.telamon.settings`:

- `open` `[<link>]`: that page, scrolled to that setting;
- `open-app` `[<desktop file ID>]`: the Apps page at App Permissions (the
  ID is only checked for form; permissions are picked there for Flatpak
  apps).

Both are read in Rust (`launch::action_args`) like any launch argument, so a
link that is not one is refused and the window only comes forward. When
Settings is not running, the session bus starts it
(`net.eterneon.telamon.settings.service`, `DBusActivatable=true`) and the call
reaches it once it is up.

The image's own changes (menu entry, dock pin, scripts) are the Telamon OS
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
  TelamonSettings).

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
