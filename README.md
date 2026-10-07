# Settings

The settings app of [AtlasOS](https://github.com/EternalCoder454/AtlasOS). It
replaces KDE System Settings with fifteen pages instead of dozens: Home,
Network, Bluetooth & Devices, Displays, Sound, Keyboard & Mouse, Appearance,
Notifications, Apps, Privacy & Security, Users, Power & Battery,
Accessibility, Time & Language and System.

- One window, its pages down a sidebar. Each page shows the settings most
  people change; the rest fold under Advanced. Search finds every setting.
- Changes go through the system's own services (NetworkManager, BlueZ,
  AccountsService, timedated and others), which ask for a password when one
  is needed. Settings has no privileged helper of its own.
- Plasma settings it has no page or row for are under Other Plasma Settings
  and open Plasma's own page.
- Other apps can open a page: `atlas-settings devices`,
  `atlas-settings --kcm kcm_bluetooth`, `atlas-settings --search "dark mode"`.

Built with Rust, Qt 6 Quick and Kirigami on
[atlas-framework](https://github.com/EternalCoder454/atlas-framework). See
`CLAUDE.md` for how to build and test it, and `docs/DESIGN.md` for how it
works.

Licence: MIT.
