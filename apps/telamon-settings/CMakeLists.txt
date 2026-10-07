cmake_minimum_required(VERSION 3.24)
project(telamon-settings VERSION 0.3.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
    set(CMAKE_INSTALL_PREFIX /usr CACHE PATH "" FORCE)
endif()
include(GNUInstallDirs)
find_package(Qt6 6.11 REQUIRED COMPONENTS Core DBus Gui Qml Quick QuickControls2 Widgets)
qt_policy(SET QTP0001 NEW)
qt_policy(SET QTP0004 NEW)
# telamon-framework-ui's startup code (single instance, window activation);
# KIO starts kcmshell6 and Telamon Updater with an activation token (the
# window from KGuiAddons' KJobWindows), and
# KCoreAddons reads the KCMs' metadata.
find_package(KF6CoreAddons REQUIRED)
find_package(KF6DBusAddons REQUIRED)
find_package(KF6WindowSystem REQUIRED)
find_package(KF6KIO REQUIRED)
find_package(KF6GuiAddons REQUIRED)
# Plasma's language and formats (plasma-localerc), written as its KCM does.
find_package(KF6Config REQUIRED)
# Displays: the screens through libkscreen, as Plasma's Display Configuration
# reads and sets them; Night Light writes kwinrc through KConfig.
find_package(KF6Screen REQUIRED)
# The installed apps (default apps, startup apps): KApplicationTrader.
find_package(KF6Service REQUIRED)
find_package(Corrosion REQUIRED)
# The Updates page's Flatpak updates: telamon-framework-flatpak links
# libflatpak and GLib into the Rust library.
find_package(PkgConfig REQUIRED)
pkg_check_modules(FLATPAK REQUIRED IMPORTED_TARGET flatpak gio-2.0)

set(WORKSPACE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../..)

# Rust library (CXX-Qt QObjects, atlas-framework, settings-registry, settings-sys).
# cxx-qt-build finds Qt via $QMAKE.
find_program(QMAKE6 NAMES qmake6 qmake REQUIRED)
corrosion_import_crate(MANIFEST_PATH ${WORKSPACE_DIR}/Cargo.toml
    CRATES telamon-settings LOCKED)
corrosion_set_env_vars(telamon_settings QMAKE=${QMAKE6})

# The framework's C header (telamon/app.h) comes with the crate's sources,
# wherever cargo put them (a git checkout in ~/.cargo).
find_program(CARGO NAMES cargo REQUIRED)
execute_process(
    COMMAND ${CARGO} metadata --format-version 1 --locked --manifest-path ${WORKSPACE_DIR}/Cargo.toml
    OUTPUT_VARIABLE CARGO_METADATA
    RESULT_VARIABLE CARGO_METADATA_RESULT
    ERROR_VARIABLE CARGO_METADATA_ERROR
    TIMEOUT 300)
if(NOT CARGO_METADATA_RESULT EQUAL 0)
    message(FATAL_ERROR "cargo metadata failed (${CARGO_METADATA_RESULT}): ${CARGO_METADATA_ERROR}\n"
        "Run `cargo fetch --locked` first, or check the network.")
endif()
string(JSON NPACKAGES LENGTH "${CARGO_METADATA}" packages)
math(EXPR LAST_PACKAGE "${NPACKAGES} - 1")
set(TELAMON_UI_CRATE_DIR "")
foreach(i RANGE ${LAST_PACKAGE})
    string(JSON PKG_NAME GET "${CARGO_METADATA}" packages ${i} name)
    if(PKG_NAME STREQUAL "telamon-framework-ui")
        string(JSON PKG_MANIFEST GET "${CARGO_METADATA}" packages ${i} manifest_path)
        get_filename_component(TELAMON_UI_CRATE_DIR "${PKG_MANIFEST}" DIRECTORY)
    endif()
endforeach()
if(NOT EXISTS "${TELAMON_UI_CRATE_DIR}/include/telamon/app.h")
    message(FATAL_ERROR "telamon-framework-ui's include/telamon/app.h not found (looked in '${TELAMON_UI_CRATE_DIR}')")
endif()

# Telamon.Ui, the shared Atlas look, is installed like Kirigami (telamon-ui, from
# atlas-framework): QML imports it and nothing links it. qmlcachegen and
# qmllint read its types from the installed copy.
if(NOT EXISTS "${QT6_INSTALL_PREFIX}/${QT6_INSTALL_QML}/Telamon/Ui/qmldir")
    message(FATAL_ERROR "Telamon.Ui is not installed: install telamon-ui (atlas-framework)")
endif()

qt_add_executable(telamon-settings-app
    cpp/main.cpp
    cpp/launcher.cpp
    cpp/legacyservice.cpp
    cpp/kcmcatalog.cpp
    cpp/localeconfig.cpp
    cpp/colorscheme.cpp
    cpp/pagebackends.cpp
    cpp/appearanceconfig.cpp
    cpp/inputconfig.cpp
    cpp/notificationsconfig.cpp
    cpp/accessibilityconfig.cpp
    cpp/powerconfig.cpp
    cpp/screenlockconfig.cpp
    cpp/autostartconfig.cpp
    cpp/defaultapps.cpp)
set_target_properties(telamon-settings-app PROPERTIES OUTPUT_NAME telamon-settings)
target_include_directories(telamon-settings-app PRIVATE "${TELAMON_UI_CRATE_DIR}/include")

# qmlcachegen compiles every QML file to C++ at build time; the app reads no
# .qml from disk at run time.
# dates.js only holds date helpers: qmllint has nothing to say about it
set_source_files_properties(qml/dates.js PROPERTIES QT_QML_SKIP_QMLLINT TRUE)
qt_add_qml_module(telamon-settings-app
    URI net.eterneon.telamon.settings
    VERSION 1.0
    RESOURCES
        qml/icons/update-arrow.svg
    QML_FILES
        qml/Main.qml
        qml/PendingPage.qml
        qml/MoreSettingsPage.qml
        qml/SearchPage.qml
        qml/SettingsPage.qml
        qml/AdvancedSection.qml
        qml/KcmRow.qml
        qml/RelatedLinks.qml
        qml/TimeLanguagePage.qml
        qml/PickerSheet.qml
        qml/SystemPage.qml
        qml/UpdatesPage.qml
        qml/CrashReportsSheet.qml
        qml/dates.js
        qml/DisplaysPage.qml
        qml/ArrangementView.qml
        qml/SoundPage.qml
        qml/NetworkPage.qml
        qml/DevicesPage.qml
        qml/HomePage.qml
        qml/AppearancePage.qml
        qml/DesktopPreview.qml
        qml/InputPage.qml
        qml/NotificationsPage.qml
        qml/AccessibilityPage.qml
        qml/PowerPage.qml
        qml/UsersPage.qml
        qml/PrivacyPage.qml
        qml/AppsPage.qml
        qml/TimeoutSheet.qml
)

target_link_libraries(telamon-settings-app PRIVATE
    telamon_settings PkgConfig::FLATPAK
    Qt6::Core Qt6::DBus Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::Widgets
    KF6::CoreAddons KF6::DBusAddons KF6::WindowSystem KF6::KIOGui KF6::GuiAddons KF6::ConfigCore KF6::ConfigGui KF6::Service
    # crypt(3), for the password hash AccountsService takes (settings-sys).
    crypt)

# Tests of the page backends that write Plasma's settings: against a temporary
# config folder and fake KWin and Plasma shell services on a private session
# bus (dbus-run-session). `ctest --test-dir <build>`.
include(CTest)
if(BUILD_TESTING)
    find_package(Qt6 6.11 REQUIRED COMPONENTS Test)
    find_program(DBUS_RUN_SESSION dbus-run-session)
    add_executable(pageconfig_test
        tests/pageconfig_test.cpp
        tests/fakes.h
        cpp/appearanceconfig.cpp
        cpp/inputconfig.cpp
        cpp/notificationsconfig.cpp
        cpp/accessibilityconfig.cpp)
    target_include_directories(pageconfig_test PRIVATE cpp)
    target_link_libraries(pageconfig_test PRIVATE Qt6::Core Qt6::DBus Qt6::Gui Qt6::Test KF6::ConfigCore KF6::ConfigGui)
    set_target_properties(pageconfig_test PROPERTIES AUTOMOC ON)
    # The old D-Bus name's forwarding (legacyservice.h), on the same private bus.
    add_executable(legacyservice_test tests/legacyservice_test.cpp cpp/legacyservice.cpp cpp/legacyservice.h)
    target_include_directories(legacyservice_test PRIVATE cpp)
    target_link_libraries(legacyservice_test PRIVATE Qt6::Core Qt6::DBus Qt6::Test)
    set_target_properties(legacyservice_test PROPERTIES AUTOMOC ON)
    # Stands in for KWin and Plasma's shell in the smoke runs (never installed).
    add_executable(fake_desktop tests/fake_desktop.cpp tests/fakes.h)
    target_link_libraries(fake_desktop PRIVATE Qt6::Core Qt6::DBus)
    set_target_properties(fake_desktop PROPERTIES AUTOMOC ON)
    if(DBUS_RUN_SESSION)
        add_test(NAME pageconfig COMMAND ${DBUS_RUN_SESSION} -- $<TARGET_FILE:pageconfig_test>)
        add_test(NAME legacyservice COMMAND ${DBUS_RUN_SESSION} -- $<TARGET_FILE:legacyservice_test>)
    else()
        add_test(NAME pageconfig COMMAND pageconfig_test)
        add_test(NAME legacyservice COMMAND legacyservice_test)
    endif()
endif()

# Displays' C++: the screens (libkscreen) and Night Light (kwinrc). A library
# of its own so the tests link what the app links.
find_package(Qt6 6.11 REQUIRED COMPONENTS DBus Test)
add_library(telamon-settings-text STATIC cpp/safetext.cpp)
target_include_directories(telamon-settings-text PUBLIC cpp)
target_link_libraries(telamon-settings-text PUBLIC Qt6::Core)
add_library(telamon-settings-displays STATIC
    cpp/displaylogic.cpp
    cpp/reverttimer.cpp
    cpp/screenconfig.cpp
    cpp/nightlight.cpp)
target_include_directories(telamon-settings-displays PUBLIC cpp)
target_link_libraries(telamon-settings-displays PUBLIC telamon-settings-text Qt6::Core Qt6::Gui Qt6::DBus KF6::ConfigCore KF6::Screen)
# TELAMON_SETTINGS_FAKE_DISPLAYS=<json>: libkscreen's Fake backend instead of the
# compositor, for the tests and the screenshots. It needs a private header of
# libkscreen (backendmanager_p.h), so a libkscreen that drops it builds with
# -DTELAMON_SETTINGS_TEST_HOOKS=OFF.
option(TELAMON_SETTINGS_TEST_HOOKS "Build the fake-screens hook for tests" ON)
if(TELAMON_SETTINGS_TEST_HOOKS)
    target_compile_definitions(telamon-settings-displays PUBLIC TELAMON_SETTINGS_TEST_HOOKS)
endif()
target_link_libraries(telamon-settings-app PRIVATE telamon-settings-displays)

# Sound's C++: PulseAudio's API on PipeWire, through PulseAudioQt (Plasma's own
# library for it).
find_package(KF6PulseAudioQt REQUIRED)
add_library(telamon-settings-sound STATIC
    cpp/soundlogic.cpp
    cpp/soundmixer.cpp)
target_include_directories(telamon-settings-sound PUBLIC cpp)
target_link_libraries(telamon-settings-sound PUBLIC telamon-settings-text Qt6::Core Qt6::Gui KF6::PulseAudioQt)
target_link_libraries(telamon-settings-app PRIVATE telamon-settings-sound)

# The C++ logic's tests (Qt Test, offscreen): `ctest --test-dir <build>`.
option(TELAMON_SETTINGS_TESTS "Build the C++ tests" ON)
if(TELAMON_SETTINGS_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

install(TARGETS telamon-settings-app RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES data/net.eterneon.telamon.settings.desktop
        DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES data/net.eterneon.telamon.settings.metainfo.xml
        DESTINATION ${CMAKE_INSTALL_DATADIR}/metainfo)
install(FILES data/net.eterneon.telamon.settings.svg
        DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps)

# The KConfig-backed settings' tests: cmake -DTELAMON_SETTINGS_TESTS=ON, then ctest.
# They write only into a temporary XDG_CONFIG_HOME.
option(TELAMON_SETTINGS_TESTS "Build the KConfig tests" OFF)
if(TELAMON_SETTINGS_TESTS)
    find_package(Qt6 6.11 REQUIRED COMPONENTS Test)
    enable_testing()
    add_executable(configtests tests/configtests.cpp
        cpp/powerconfig.cpp
        cpp/screenlockconfig.cpp
        cpp/autostartconfig.cpp
        cpp/defaultapps.cpp)
    target_link_libraries(configtests PRIVATE Qt6::Core Qt6::Test
        KF6::ConfigCore KF6::CoreAddons KF6::Service)
    add_test(NAME configtests COMMAND configtests)
endif()
