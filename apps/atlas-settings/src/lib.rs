//! Rust side of Settings. `cpp/main.cpp` starts Qt and the single-instance
//! service; the pages' logic lives here as QObjects exposed to QML, over the
//! Qt-free `settings-registry` and `settings-sys`.

mod apps_page;
mod backend;
mod bluetooth;
mod network;
mod power_page;
mod privacy_page;
mod support;
mod system_info;
mod time_language;
mod users_page;
mod worker;

atlas_framework_ui::app! {
    name: "Settings",
    id: "net.eterneon.atlas.settings",
    repo: "atlasos-settings",
    ui: "1.4.0",
}

use std::ffi::{CStr, c_char, c_void};

/// Called once from `main.cpp`. Returns the `Backend` QObject, which C++ hands
/// to the QML engine. Ownership passes to the caller (a QObject with no parent).
#[unsafe(no_mangle)]
pub extern "C" fn atlas_backend_new() -> *mut c_void {
    backend::qobject::backend_make_unique().into_raw().cast()
}

/// A page's backend, made when the page is shown (`cpp/pagebackends.cpp`):
/// `time-language`, `system`, `power`, `users`, `privacy` or `apps`; null for
/// any other kind. Ownership passes to
/// the caller, which parents it to the page so it goes with it.
///
/// # Safety
/// `kind` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn atlas_page_backend_new(kind: *const c_char) -> *mut c_void {
    if kind.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the caller passes a NUL-terminated string.
    let kind = unsafe { CStr::from_ptr(kind) }.to_bytes();
    match kind {
        b"time-language" => time_language::qobject::time_language_make_unique()
            .into_raw()
            .cast(),
        b"power" => power_page::qobject::power_page_make_unique()
            .into_raw()
            .cast(),
        b"users" => users_page::qobject::users_page_make_unique()
            .into_raw()
            .cast(),
        b"privacy" => privacy_page::qobject::privacy_page_make_unique()
            .into_raw()
            .cast(),
        b"apps" => apps_page::qobject::apps_page_make_unique()
            .into_raw()
            .cast(),
        b"system" => system_info::qobject::system_info_make_unique()
            .into_raw()
            .cast(),
        b"network" => network::qobject::network_page_make_unique()
            .into_raw()
            .cast(),
        b"bluetooth" => bluetooth::qobject::bluetooth_page_make_unique()
            .into_raw()
            .cast(),
        _ => std::ptr::null_mut(),
    }
}
