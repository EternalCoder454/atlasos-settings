Developer fixtures for the Updates page: what the update helper, Flatpak,
fwupd and the release notes would answer in each state, copied from
atlasos-updater (`crates/telamon-updater-core/fixtures-states`, where Telamon Updater's
own window was tested). `TELAMON_UPDATER_FIXTURES=<one of these folders>`
(a debug build) shows the page with them, never touching the real system
(see docs/DESIGN.md, "Updates"; scripts/smoke.sh).
