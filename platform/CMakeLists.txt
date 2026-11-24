# Projet autonome qui construit et teste les bibliotheques de plateforme,
# sans widget ni Slint. C est ici qu on lance ctest.
cmake_minimum_required(VERSION 3.21)
project(osampo_platform LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(${CMAKE_CURRENT_SOURCE_DIR}/../cmake/cjson.cmake)

enable_testing()

add_subdirectory(link_common)

add_subdirectory(link_socket)

add_executable(test_json_line link_common/test/test_json_line.cpp)
target_link_libraries(test_json_line PRIVATE link_common)
add_test(NAME json_line COMMAND test_json_line)

add_executable(test_socket_link link_socket/test/test_socket_link.cpp)
target_link_libraries(test_socket_link PRIVATE link_socket)
add_test(NAME socket_link COMMAND test_socket_link)
set_tests_properties(socket_link PROPERTIES TIMEOUT 40)
set_tests_properties(json_line PROPERTIES TIMEOUT 10)
