cmake_minimum_required(VERSION 3.22)
cmake_policy(SET CMP0091 NEW)

if (APPLE)
    set(CMAKE_OSX_DEPLOYMENT_TARGET "12.0" CACHE STRING "Minimum macOS version" FORCE)
    project(EquaformPlugins VERSION 1.0.0 LANGUAGES C CXX OBJC OBJCXX)
    set(CLAP_WRAPPER_BUILD_AUV2 ON CACHE BOOL "" FORCE)
else()
    project(EquaformPlugins VERSION 1.0.0 LANGUAGES C CXX)
    set(CLAP_WRAPPER_BUILD_AUV2 OFF CACHE BOOL "" FORCE)
endif()

if (MSVC AND NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
    # Match clap-wrapper's static CRT (/MT) across both parent and subdirectory scopes
    # so equaform_vst3 and base-sdk-vst3 never mismatch (LNK2038).
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

set(CMAKE_CXX_STANDARD 20 CACHE STRING "C++ standard" FORCE)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(CLAP_WRAPPER_OUTPUT_NAME "Equaform-8" CACHE STRING "" FORCE)
set(CLAP_WRAPPER_BUNDLE_IDENTIFIER "com.equaform.audio.equalizer" CACHE STRING "" FORCE)
set(CLAP_WRAPPER_BUNDLE_VERSION "1.0.0" CACHE STRING "" FORCE)
set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES ON CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_COPY_AFTER_BUILD OFF CACHE BOOL "" FORCE)

set(EQUAFORM_CLAP_BUNDLE "" CACHE PATH "Path to compiled Equaform-8.clap bundle to embed")
set(EQUAFORM_BUNDLE_OUT_DIR "${CMAKE_BINARY_DIR}/bundles" CACHE PATH "Output directory for built plugin bundles")

add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../clap-wrapper" "${CMAKE_BINARY_DIR}/clap-wrapper")

# ---------------------------------------------------------------------------
# VST3 Wrapper (Ableton Live, Cubase, Reaper, Bitwig, FL Studio)
# ---------------------------------------------------------------------------
add_library(equaform_vst3 MODULE)
set_target_properties(equaform_vst3 PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${EQUAFORM_BUNDLE_OUT_DIR}"
)
target_add_vst3_wrapper(
    TARGET equaform_vst3
    OUTPUT_NAME "Equaform-8"
    BUNDLE_IDENTIFIER "com.equaform.audio.equalizer"
    BUNDLE_VERSION "1.0.0"
    MACOS_EMBEDDED_CLAP_LOCATION "${EQUAFORM_CLAP_BUNDLE}"
    WINDOWS_FOLDER_VST3 ON
    ASSET_OUTPUT_DIRECTORY "${EQUAFORM_BUNDLE_OUT_DIR}"
)

# ---------------------------------------------------------------------------
# AUv2 Wrapper (macOS only: Logic Pro, GarageBand, MainStage)
# Type: 'aufx' (Audio Unit Effect), Subtype: 'Eqf8', Manufacturer: 'Eqfm'
# ---------------------------------------------------------------------------
if (APPLE)
    add_library(equaform_auv2 MODULE)
    set_target_properties(equaform_auv2 PROPERTIES
        LIBRARY_OUTPUT_DIRECTORY "${EQUAFORM_BUNDLE_OUT_DIR}"
    )
    target_add_auv2_wrapper(
        TARGET equaform_auv2
        OUTPUT_NAME "Equaform-8"
        BUNDLE_IDENTIFIER "com.equaform.audio.equalizer"
        BUNDLE_VERSION "1.0.0"
        MANUFACTURER_NAME "Equaform Audio"
        MANUFACTURER_CODE "Eqfm"
        SUBTYPE_CODE "Eqf8"
        INSTRUMENT_TYPE "aufx"
        MACOS_EMBEDDED_CLAP_LOCATION "${EQUAFORM_CLAP_BUNDLE}"
    )
endif()
