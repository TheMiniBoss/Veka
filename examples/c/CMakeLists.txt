# Les exemples en C.
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

# Les exemples : compteur (étape 1), fenetre_perso (étape 2), colora (étape 3), anima (étape 4) et
# boutique (étape 5, le langage .vk), verre (étape 6), fusion (étape 8), formulaire (étape 9) et liste (étape 10).
foreach(exemple compteur fenetre_perso colora anima boutique verre fusion formulaire liste)
    add_executable(${exemple} ${exemple}.c)
    target_link_libraries(${exemple} PRIVATE veka)
    if(MSVC)
        target_compile_options(${exemple} PRIVATE /W4 /utf-8)
    else()
        target_compile_options(${exemple} PRIVATE -Wall -Wextra -pedantic)
    endif()
    if(WIN32)
        # La DLL à côté de l'exécutable.
        add_custom_command(TARGET ${exemple} POST_BUILD
            COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${VEKA_LIB}" "$<TARGET_FILE_DIR:${exemple}>")
    else()
        set_target_properties(${exemple} PROPERTIES BUILD_RPATH "${VEKA_LIB_DIR}")
    endif()
endforeach()
# `boutique` lit `boutique.vk` dans ce dossier.
target_compile_definitions(boutique PRIVATE VEKA_VK_DIR="${CMAKE_CURRENT_SOURCE_DIR}")
