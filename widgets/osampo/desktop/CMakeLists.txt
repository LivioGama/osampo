cmake_minimum_required(VERSION 3.21)
project(osampo LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Remonte jusqu a la racine du depot, reperee par un fichier marqueur :
# pas de chemin relatif en dur.
set(_d ${CMAKE_CURRENT_SOURCE_DIR})
while(NOT EXISTS ${_d}/.osampo_root)
    get_filename_component(_p ${_d} DIRECTORY)
    if(_p STREQUAL _d)
        message(FATAL_ERROR "racine du depot introuvable : fichier .osampo_root absent")
    endif()
    set(_d ${_p})
endwhile()
set(BOARDS_ROOT ${_d})
message(STATUS "racine du depot: ${BOARDS_ROOT}")

include(${BOARDS_ROOT}/cmake/cjson.cmake)
add_subdirectory(${BOARDS_ROOT}/platform/link_common ${CMAKE_BINARY_DIR}/link_common)
add_subdirectory(${BOARDS_ROOT}/platform/link_socket ${CMAKE_BINARY_DIR}/link_socket)

include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG release/1
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)

add_executable(osampo
    main.cpp
    sound_desktop.cpp
    ../src/fake_source.cpp
    ../src/decode.cpp
    ../src/logic/controller.cpp
    ../src/logic/pup.cpp
    ../src/logic/reminders.cpp
    ../src/logic/clock.cpp
    ../src/logic/needs.cpp
    ../src/logic/settings.cpp ../src/logic/rappel.cpp
    ../src/logic/lien_pc.cpp
    ../src/logic/volume.cpp
)

target_include_directories(osampo PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
# Les WAV sont lus en place par paplay (desktop seulement).
get_filename_component(OSAMPO_SOUNDS_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../sounds ABSOLUTE)
target_compile_definitions(osampo PRIVATE OSAMPO_SOUNDS_DIR="${OSAMPO_SOUNDS_DIR}")

slint_target_sources(osampo
    ../ui/app.slint
)

target_link_libraries(osampo PRIVATE Slint::Slint link_socket link_common)

enable_testing()
add_executable(test_decode ../src/test/test_decode.cpp ../src/decode.cpp)
target_include_directories(test_decode PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_link_libraries(test_decode PRIVATE link_common)
add_test(NAME decode COMMAND test_decode)

add_executable(test_clock ../src/test/test_clock.cpp ../src/logic/clock.cpp ../src/logic/settings.cpp ../src/logic/rappel.cpp)
target_include_directories(test_clock PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_clock PRIVATE -Wall -Wextra)
target_link_libraries(test_clock PRIVATE link_common)
add_test(NAME clock COMMAND test_clock)

add_executable(test_reminders ../src/test/test_reminders.cpp ../src/logic/reminders.cpp ../src/logic/clock.cpp ../src/logic/settings.cpp ../src/logic/rappel.cpp)
target_include_directories(test_reminders PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_reminders PRIVATE -Wall -Wextra)
target_link_libraries(test_reminders PRIVATE link_common)
add_test(NAME reminders COMMAND test_reminders)

add_executable(test_volume ../src/test/test_volume.cpp ../src/logic/volume.cpp)
target_include_directories(test_volume PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_volume PRIVATE -Wall -Wextra)
target_link_libraries(test_volume PRIVATE link_common)
add_test(NAME volume COMMAND test_volume)

add_executable(test_needs ../src/test/test_needs.cpp ../src/logic/needs.cpp ../src/logic/rappel.cpp)
target_include_directories(test_needs PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_needs PRIVATE -Wall -Wextra)
target_link_libraries(test_needs PRIVATE link_common)
add_test(NAME needs COMMAND test_needs)

add_executable(test_pup ../src/test/test_pup.cpp ../src/logic/pup.cpp ../src/logic/reminders.cpp ../src/logic/clock.cpp ../src/logic/needs.cpp ../src/logic/settings.cpp ../src/logic/rappel.cpp)
target_include_directories(test_pup PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_pup PRIVATE -Wall -Wextra)
target_link_libraries(test_pup PRIVATE link_common)
add_test(NAME pup COMMAND test_pup)

add_executable(test_lien_pc ../src/test/test_lien_pc.cpp ../src/logic/lien_pc.cpp)
target_include_directories(test_lien_pc PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_lien_pc PRIVATE -Wall -Wextra)
target_link_libraries(test_lien_pc PRIVATE link_common)
add_test(NAME lien_pc COMMAND test_lien_pc)

add_executable(test_controller ../src/test/test_controller.cpp ../src/logic/controller.cpp ../src/logic/lien_pc.cpp ../src/logic/pup.cpp ../src/logic/reminders.cpp ../src/logic/clock.cpp ../src/logic/needs.cpp ../src/logic/settings.cpp ../src/logic/rappel.cpp)
target_include_directories(test_controller PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_controller PRIVATE -Wall -Wextra)
target_link_libraries(test_controller PRIVATE link_common)
add_test(NAME controller COMMAND test_controller)

add_executable(test_settings ../src/test/test_settings.cpp ../src/logic/settings.cpp ../src/logic/rappel.cpp)
target_include_directories(test_settings PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_settings PRIVATE -Wall -Wextra)
target_compile_definitions(test_settings PRIVATE CFG_DEFAUT_JSON="${CMAKE_CURRENT_SOURCE_DIR}/../src/test/cfg_defaut.json")
target_link_libraries(test_settings PRIVATE link_common)
add_test(NAME settings COMMAND test_settings)

add_executable(test_gestures ../src/test/test_gestures.cpp ../src/logic/gestures.cpp ../src/logic/rappel.cpp)
target_include_directories(test_gestures PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_gestures PRIVATE -Wall -Wextra)
target_compile_definitions(test_gestures PRIVATE TRACES_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../src/test/traces")
target_link_libraries(test_gestures PRIVATE link_common)
add_test(NAME gestures COMMAND test_gestures)

add_executable(test_allure ../src/test/test_allure.cpp ../src/logic/allure.cpp)
target_include_directories(test_allure PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../src)
target_compile_options(test_allure PRIVATE -Wall -Wextra)
target_compile_definitions(test_allure PRIVATE TRACES_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../src/test/traces")
add_test(NAME allure COMMAND test_allure)
