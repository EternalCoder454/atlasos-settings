cmake_minimum_required(VERSION 3.24)
project(atlas-settings VERSION 0.1.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
    set(CMAKE_INSTALL_PREFIX /usr CACHE PATH "" FORCE)
endif()
include(GNUInstallDirs)
find_package(Qt6 6.11 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Widgets)
qt_policy(SET QTP0001 NEW)
qt_policy(SET QTP0004 NEW)
# atlas-framework-ui's startup code (single instance, window activation);
# KIO starts kcmshell6 and Atlas Updater with an activation token (the
# window from KGuiAddons' KJobWindows), and
# KCoreAddons reads the KCMs' metadata.
find_package(KF6CoreAddons REQUIRED)
find_package(KF6DBusAddons REQUIRED)
find_package(KF6WindowSystem REQUIRED)
find_package(KF6KIO REQUIRED)
find_package(KF6GuiAddons REQUIRED)
# Plasma's language and formats (plasma-localerc), written as its KCM does.
find_package(KF6Config REQUIRED)
# The installed apps (default apps, startup apps): KApplicationTrader.
find_package(KF6Service REQUIRED)
find_package(Corrosion REQUIRED)

set(WORKSPACE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../..)

# Rust library (CXX-Qt QObjects, atlas-framework, settings-registry, settings-sys).
# cxx-qt-build finds Qt via $QMAKE.
find_program(QMAKE6 NAMES qmake6 qmake REQUIRED)
corrosion_import_crate(MANIFEST_PATH ${WORKSPACE_DIR}/Cargo.toml
    CRATES atlas-settings LOCKED)
corrosion_set_env_vars(atlas_settings QMAKE=${QMAKE6})

# The framework's C header (atlas/app.h) comes with the crate's sources,
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
set(ATLAS_UI_CRATE_DIR "")
foreach(i RANGE ${LAST_PACKAGE})
    string(JSON PKG_NAME GET "${CARGO_METADATA}" packages ${i} name)
    if(PKG_NAME STREQUAL "atlas-framework-ui")
        string(JSON PKG_MANIFEST GET "${CARGO_METADATA}" packages ${i} manifest_path)
        get_filename_component(ATLAS_UI_CRATE_DIR "${PKG_MANIFEST}" DIRECTORY)
    endif()
endforeach()
if(NOT EXISTS "${ATLAS_UI_CRATE_DIR}/include/atlas/app.h")
    message(FATAL_ERROR "atlas-framework-ui's include/atlas/app.h not found (looked in '${ATLAS_UI_CRATE_DIR}')")
endif()

# Atlas.Ui, the shared Atlas look, is installed like Kirigami (atlas-ui, from
# atlas-framework): QML imports it and nothing links it. qmlcachegen and
# qmllint read its types from the installed copy.
if(NOT EXISTS "${QT6_INSTALL_PREFIX}/${QT6_INSTALL_QML}/Atlas/Ui/qmldir")
    message(FATAL_ERROR "Atlas.Ui is not installed: install atlas-ui (atlas-framework)")
endif()

qt_add_executable(atlas-settings-app
    cpp/main.cpp
    cpp/launcher.cpp
    cpp/kcmcatalog.cpp
    cpp/localeconfig.cpp
    cpp/powerconfig.cpp
    cpp/screenlockconfig.cpp
    cpp/autostartconfig.cpp
    cpp/defaultapps.cpp
    cpp/pagebackends.cpp)
set_target_properties(atlas-settings-app PROPERTIES OUTPUT_NAME atlas-settings)
target_include_directories(atlas-settings-app PRIVATE "${ATLAS_UI_CRATE_DIR}/include")

# qmlcachegen compiles every QML file to C++ at build time; the app reads no
# .qml from disk at run time.
qt_add_qml_module(atlas-settings-app
    URI net.eterneon.atlas.settings
    VERSION 1.0
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
        qml/PowerPage.qml
        qml/UsersPage.qml
        qml/PrivacyPage.qml
        qml/TimeoutSheet.qml
)

target_link_libraries(atlas-settings-app PRIVATE
    atlas_settings
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::Widgets
    KF6::CoreAddons KF6::DBusAddons KF6::WindowSystem KF6::KIOGui KF6::GuiAddons KF6::ConfigCore KF6::Service
    # crypt(3), for the password hash AccountsService takes (settings-sys).
    crypt)

install(TARGETS atlas-settings-app RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES data/net.eterneon.atlas.settings.desktop
        DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES data/net.eterneon.atlas.settings.metainfo.xml
        DESTINATION ${CMAKE_INSTALL_DATADIR}/metainfo)
install(FILES data/net.eterneon.atlas.settings.svg
        DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps)

# The KConfig-backed settings' tests: cmake -DATLAS_SETTINGS_TESTS=ON, then ctest.
# They write only into a temporary XDG_CONFIG_HOME.
option(ATLAS_SETTINGS_TESTS "Build the KConfig tests" OFF)
if(ATLAS_SETTINGS_TESTS)
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
