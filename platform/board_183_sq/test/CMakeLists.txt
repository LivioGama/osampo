cmake_minimum_required(VERSION 3.16)
project(board_183_sq_tests CXX)

# Tests hote de la logique pure de board_183_sq : aucun en-tete ESP-IDF.
# Autonome, hors de platform/CMakeLists.txt.
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
# assert doit rester actif quel que soit le type de build
string(REPLACE "-DNDEBUG" "" CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE}")

enable_testing()

set(LOGIC ${CMAKE_CURRENT_SOURCE_DIR}/../logic)

add_executable(test_logic test_logic.cpp
    ${LOGIC}/pcf85063.cpp ${LOGIC}/wav.cpp ${LOGIC}/line_buffer.cpp ${LOGIC}/axp2101.cpp ${LOGIC}/es8311.cpp ${LOGIC}/vbat_log.cpp ${LOGIC}/nvs_migration.cpp)
target_include_directories(test_logic PRIVATE ${LOGIC})
target_compile_options(test_logic PRIVATE -Wall -Wextra)
add_test(NAME test_logic COMMAND test_logic)
