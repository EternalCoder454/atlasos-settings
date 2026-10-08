# Run by ctest (cmake -P): no desktop file Settings ships carries
# StartupWMClass. The window's app id is net.eterneon.telamon.settings, the name
# of the real desktop file, which Plasma's task manager matches to the dock pin
# on its own. A hidden file (systemsettings.desktop and the like) that also
# named it as its window class won the match, and Settings opened as a second
# icon beside its pin.
file(GLOB desktop_files "${DATA_DIR}/*.desktop")
if(NOT desktop_files)
    message(FATAL_ERROR "no desktop files in ${DATA_DIR}")
endif()
set(bad "")
foreach(f IN LISTS desktop_files)
    file(STRINGS "${f}" wmclass REGEX "^StartupWMClass=")
    if(wmclass)
        list(APPEND bad "${f}")
    endif()
endforeach()
if(bad)
    message(FATAL_ERROR "StartupWMClass would make Plasma group Settings' window with the wrong launcher: ${bad}")
endif()
