Developer fixtures for the Updates page: what the update helper, Flatpak,
fwupd and the release notes would answer in each state, copied from
atlasos-updater (`crates/telamon-updater-core/fixtures-states`, where Telamon Updater's
own window was tested). `TELAMON_UPDATER_FIXTURES=<one of these folders>`
(a debug build) shows the page with them, never touching the real system
(see docs/DESIGN.md, "Updates"; scripts/smoke.sh).

`dmi/` holds the kernel's DMI files (`bios_vendor`, `bios_version`,
`bios_date`, `sys_vendor`, `board_vendor`, `board_name`) for the computer's own
firmware row, and `system-firmware-verdict` what fwupd knows of it (`upToDate`,
`updateAvailable`, `noMetadata` or `unknown`). `no-firmware-metadata` is the
state of a PC whose maker publishes nothing to fwupd.
