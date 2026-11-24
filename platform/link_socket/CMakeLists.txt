# Client du socket du daemon, cible desktop. Aucune dependance a Slint.
add_library(link_socket STATIC socket_link.cpp)
target_include_directories(link_socket PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_features(link_socket PUBLIC cxx_std_17)
target_link_libraries(link_socket PUBLIC link_common)
# socket_link.cpp construit le hello avec cJSON : dependance directe, explicite.
target_link_libraries(link_socket PRIVATE cjson)
find_package(Threads REQUIRED)
target_link_libraries(link_socket PUBLIC Threads::Threads)
