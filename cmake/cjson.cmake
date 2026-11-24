# Recette cJSON pour les cibles desktop.
#
# Le CMakeLists amont de cJSON est inutilisable tel quel, pour quatre raisons
# mesurees sur cette machine avec CMake 3.28.3 :
#   1. la cible cjson ne propage aucun repertoire d inclusion ;
#   2. ENABLE_CJSON_TEST est ON par defaut et fait entrer toute la suite amont ;
#   3. BUILD_SHARED_LIBS est ON par defaut ;
#   4. il declare cmake_minimum_required(VERSION 3.0), erreur dure des CMake 4.
#
# On saute donc son CMakeLists, via un SOURCE_SUBDIR qui n existe pas, et on
# compile le fichier source directement.
#
# Version epinglee sur celle qu embarque ESP-IDF v5.5.4, pour n avoir aucun
# ecart entre la cible desktop et la cible esp32s3.

include_guard(GLOBAL)
include(FetchContent)

FetchContent_Declare(cjson
    GIT_REPOSITORY https://github.com/DaveGamble/cJSON.git
    GIT_TAG v1.7.19
    SOURCE_SUBDIR pas-de-cmake-ici
)
FetchContent_MakeAvailable(cjson)

if(NOT TARGET cjson)
    add_library(cjson STATIC ${cjson_SOURCE_DIR}/cJSON.c)
    target_include_directories(cjson PUBLIC ${cjson_SOURCE_DIR})
endif()
