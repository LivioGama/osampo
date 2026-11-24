# Decodage d une ligne JSON, partage par link_socket et, plus tard, link_ble.
add_library(link_common STATIC json_line.cpp)
target_include_directories(link_common PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_features(link_common PUBLIC cxx_std_17)
target_link_libraries(link_common PUBLIC cjson)
