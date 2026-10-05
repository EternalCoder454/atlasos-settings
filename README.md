# Settings

The settings app of [AtlasOS](https://github.com/EternalCoder454/AtlasOS). It
replaces KDE System Settings: Wi-Fi and network, Bluetooth, displays, sound,
appearance, the desktop and dock, notifications, power, users, date and time,
language and region, keyboard, mouse and touchpad, printers, default apps,
app permissions, privacy, accessibility, the firewall, updates and about.

- One window, its pages in groups down a sidebar, with search over every page
  and the settings on it.
- Changes go through the system's own services (NetworkManager, BlueZ,
  AccountsService, timedated and others), which ask for a password when one
  is needed. Settings has no privileged helper of its own.
- Settings it has no page for are under More Settings and open Plasma's own
  page.
- Other apps can open a page: `atlas-settings bluetooth`,
  `atlas-settings --kcm kcm_bluetooth`, `atlas-settings --search "dark mode"`.

Built with Rust, Qt 6 Quick and Kirigami on
[atlas-framework](https://github.com/EternalCoder454/atlas-framework). See
`CLAUDE.md` for how to build and test it, and `docs/DESIGN.md` for how it
works.

Licence: MIT.
