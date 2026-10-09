# Qt Test, run offscreen. The screens test reads libkscreen's Fake backend's
# JSON (fixtures/), never the real screens; Night Light's test writes a kwinrc
# under Qt's test-mode config dir.
function(telamon_settings_test name)
    add_executable(${name} ${name}.cpp)
    target_link_libraries(${name} PRIVATE telamon-settings-displays Qt6::Test)
    add_test(NAME ${name} COMMAND ${name})
    set_tests_properties(${name} PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;TELAMON_FIXTURES=${CMAKE_CURRENT_SOURCE_DIR}/fixtures")
endfunction()

function(telamon_settings_sound_test name)
    add_executable(${name} ${name}.cpp)
    target_link_libraries(${name} PRIVATE telamon-settings-sound Qt6::Test)
    add_test(NAME ${name} COMMAND ${name})
    set_tests_properties(${name} PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
endfunction()

# Needs no Qt: only reads the desktop files in data/.
add_test(NAME desktop_wmclass_test
    COMMAND ${CMAKE_COMMAND} -DDATA_DIR=${CMAKE_CURRENT_SOURCE_DIR}/../data -P ${CMAKE_CURRENT_SOURCE_DIR}/desktop_wmclass_test.cmake)

telamon_settings_test(displaylogic_test)
telamon_settings_sound_test(soundlogic_test)
telamon_settings_sound_test(soundmixer_test)
telamon_settings_test(nightlight_test)
if(TELAMON_SETTINGS_TEST_HOOKS)
    # The one test that needs the fake screens links the copy of the screens'
    # code that has the hook; the app never does.
    add_executable(screenconfig_test screenconfig_test.cpp)
    target_link_libraries(screenconfig_test PRIVATE telamon-settings-displays-hooks Qt6::Test)
    add_test(NAME screenconfig_test COMMAND screenconfig_test)
    set_tests_properties(screenconfig_test PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;TELAMON_FIXTURES=${CMAKE_CURRENT_SOURCE_DIR}/fixtures")
endif()
