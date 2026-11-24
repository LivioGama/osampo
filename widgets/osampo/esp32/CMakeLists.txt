cmake_minimum_required(VERSION 3.16)

# Le composant de la board (pilote) et ses options de projet.
set(EXTRA_COMPONENT_DIRS "${CMAKE_CURRENT_LIST_DIR}/../../../platform/board_183_sq")
set(SDKCONFIG_DEFAULTS "${CMAKE_CURRENT_LIST_DIR}/../../../platform/board_183_sq/sdkconfig.defaults")

include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(osampo)
