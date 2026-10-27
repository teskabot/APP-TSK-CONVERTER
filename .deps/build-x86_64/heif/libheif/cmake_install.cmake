# Install script for directory: /Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/libheif-x86_64")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Release")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "FALSE")
endif()

# Set path to fallback-tool for dependency-resolution.
if(NOT DEFINED CMAKE_OBJDUMP)
  set(CMAKE_OBJDUMP "/usr/bin/objdump")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  # Include the install script for the subdirectory.
  include("/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/plugins/cmake_install.cmake")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/libheif.a")
  if(EXISTS "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libheif.a" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libheif.a")
    execute_process(COMMAND "/usr/bin/ranlib" "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libheif.a")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/libheif" TYPE FILE FILES
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_export.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_library.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_image.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_color.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_error.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_plugin.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_properties.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_regions.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_items.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_sequences.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_tai_timestamps.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_brands.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_metadata.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_aux_images.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_entity_groups.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_security.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_encoding.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_decoding.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_image_handle.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_context.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_tiling.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_components.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_uncompressed.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_text.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_omaf.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/src/libheif-1.23.7/libheif/api/libheif/heif_cxx.h"
    "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/heif_version.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif/libheif-config.cmake")
    file(DIFFERENT _cmake_export_file_changed FILES
         "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif/libheif-config.cmake"
         "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/CMakeFiles/Export/5cd8613eea38798f9c35b1a25e1b106b/libheif-config.cmake")
    if(_cmake_export_file_changed)
      file(GLOB _cmake_old_config_files "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif/libheif-config-*.cmake")
      if(_cmake_old_config_files)
        string(REPLACE ";" ", " _cmake_old_config_files_text "${_cmake_old_config_files}")
        message(STATUS "Old export file \"$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif/libheif-config.cmake\" will be replaced.  Removing files [${_cmake_old_config_files_text}].")
        unset(_cmake_old_config_files_text)
        file(REMOVE ${_cmake_old_config_files})
      endif()
      unset(_cmake_old_config_files)
    endif()
    unset(_cmake_export_file_changed)
  endif()
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif" TYPE FILE FILES "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/CMakeFiles/Export/5cd8613eea38798f9c35b1a25e1b106b/libheif-config.cmake")
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif" TYPE FILE FILES "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/CMakeFiles/Export/5cd8613eea38798f9c35b1a25e1b106b/libheif-config-release.cmake")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/libheif" TYPE FILE FILES "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/libheif-config-version.cmake")
endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
if(CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "/Users/tomas/Documents/Code/APP CONVERSOR TSK/conversor/.deps/build-x86_64/heif/libheif/install_local_manifest.txt"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
