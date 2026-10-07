use cxx_qt_build::CxxQtBuilder;

fn main() {
    // Generates the C++ for the QObject bridges and compiles it into the Rust
    // static library. Qt is found through $QMAKE (CMake sets it).
    CxxQtBuilder::new()
        .file("src/backend.rs")
        .file("src/time_language.rs")
        .file("src/power_page.rs")
        .file("src/users_page.rs")
        .file("src/privacy_page.rs")
        .file("src/system_info.rs")
        .build();
}
