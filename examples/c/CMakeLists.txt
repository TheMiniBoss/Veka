# L'exemple compteur en C.
#
#   cmake -S examples/c -B build/c
#   cmake --build build/c
#   build/c/compteur            (Windows : build\c\Debug\compteur.exe)
#
# CMake lance `cargo build -p veka-ffi` puis lie l'exécutable à la bibliothèque dynamique de Veka
# (veka.dll sous Windows, libveka.dylib sous macOS).
cmake_minimum_required(VERSION 3.20)
project(veka_compteur C)

set(VEKA_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../..")
set(VEKA_PROFILE "debug" CACHE STRING "Profil cargo de Veka : debug ou release")
if(VEKA_PROFILE STREQUAL "release")
    set(VEKA_CARGO_FLAGS --release)
endif()
set(VEKA_LIB_DIR "${VEKA_ROOT}/target/${VEKA_PROFILE}")

if(WIN32)
    set(VEKA_LIB "${VEKA_LIB_DIR}/veka.dll")
    set(VEKA_IMPLIB "${VEKA_LIB_DIR}/veka.dll.lib")
elseif(APPLE)
    set(VEKA_LIB "${VEKA_LIB_DIR}/libveka.dylib")
else()
    set(VEKA_LIB "${VEKA_LIB_DIR}/libveka.so")
endif()

add_custom_target(veka_cargo
    COMMAND cargo build -p veka-ffi ${VEKA_CARGO_FLAGS}
    WORKING_DIRECTORY "${VEKA_ROOT}"
    BYPRODUCTS "${VEKA_LIB}" ${VEKA_IMPLIB}
    COMMENT "cargo build -p veka-ffi"
    VERBATIM)

add_library(veka SHARED IMPORTED)
set_target_properties(veka PROPERTIES
    IMPORTED_LOCATION "${VEKA_LIB}"
    INTERFACE_INCLUDE_DIRECTORIES "${VEKA_ROOT}/crates/veka-ffi/include")
if(WIN32)
    set_target_properties(veka PROPERTIES IMPORTED_IMPLIB "${VEKA_IMPLIB}")
endif()
add_dependencies(veka veka_cargo)

add_executable(compteur compteur.c)
target_link_libraries(compteur PRIVATE veka)
if(MSVC)
    target_compile_options(compteur PRIVATE /W4 /utf-8)
else()
    target_compile_options(compteur PRIVATE -Wall -Wextra -pedantic)
endif()

if(WIN32)
    # La DLL à côté de l'exécutable.
    add_custom_command(TARGET compteur POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${VEKA_LIB}" "$<TARGET_FILE_DIR:compteur>")
else()
    set_target_properties(compteur PROPERTIES BUILD_RPATH "${VEKA_LIB_DIR}")
endif()
