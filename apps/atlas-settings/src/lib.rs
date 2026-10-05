//! Rust side of Settings. `cpp/main.cpp` starts Qt and the single-instance
//! service; the pages' logic lives here as QObjects exposed to QML, over the
//! Qt-free `settings-registry` and `settings-sys`.

mod backend;

atlas_framework_ui::app! {
    name: "Settings",
    id: "net.eterneon.atlas.settings",
    repo: "atlasos-settings",
    ui: "1.4.0",
}

use std::ffi::c_void;

/// Called once from `main.cpp`. Returns the `Backend` QObject, which C++ hands
/// to the QML engine. Ownership passes to the caller (a QObject with no parent).
#[unsafe(no_mangle)]
pub extern "C" fn atlas_backend_new() -> *mut c_void {
    backend::qobject::backend_make_unique().into_raw().cast()
}
