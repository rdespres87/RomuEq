cmake_minimum_required(VERSION 3.22)

set(CMAKE_OSX_DEPLOYMENT_TARGET "12.0" CACHE STRING "Minimum macOS version" FORCE)
set(CMAKE_CXX_STANDARD 20 CACHE STRING "C++ standard" FORCE)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

project(RomuEqPlugins VERSION 0.1.0 LANGUAGES C CXX OBJC OBJCXX)

set(CLAP_WRAPPER_OUTPUT_NAME "RomuEq" CACHE STRING "" FORCE)
set(CLAP_WRAPPER_BUNDLE_IDENTIFIER "com.romueq.plugin" CACHE STRING "" FORCE)
set(CLAP_WRAPPER_BUNDLE_VERSION "0.1.0" CACHE STRING "" FORCE)
set(CLAP_WRAPPER_BUILD_AUV2 ON CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES ON CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_COPY_AFTER_BUILD OFF CACHE BOOL "" FORCE)

set(ROMUEQ_CLAP_BUNDLE "" CACHE PATH "Path to compiled RomuEq.clap bundle to embed")
set(ROMUEQ_BUNDLE_OUT_DIR "${CMAKE_BINARY_DIR}/bundles" CACHE PATH "Output directory for built plugin bundles")

add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../clap-wrapper" "${CMAKE_BINARY_DIR}/clap-wrapper")

# ---------------------------------------------------------------------------
# VST3 Wrapper (Ableton Live, Cubase, Reaper, Bitwig)
# ---------------------------------------------------------------------------
add_library(romueq_vst3 MODULE)
set_target_properties(romueq_vst3 PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${ROMUEQ_BUNDLE_OUT_DIR}"
)
target_add_vst3_wrapper(
    TARGET romueq_vst3
    OUTPUT_NAME "RomuEq"
    BUNDLE_IDENTIFIER "com.romueq.plugin"
    BUNDLE_VERSION "0.1.0"
    MACOS_EMBEDDED_CLAP_LOCATION "${ROMUEQ_CLAP_BUNDLE}"
)

# ---------------------------------------------------------------------------
# AUv2 Wrapper (Logic Pro, GarageBand, MainStage)
# Type: 'aufx' (Audio Unit Effect), Subtype: 'RmEq', Manufacturer: 'Romu'
# ---------------------------------------------------------------------------
add_library(romueq_auv2 MODULE)
set_target_properties(romueq_auv2 PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${ROMUEQ_BUNDLE_OUT_DIR}"
)
target_add_auv2_wrapper(
    TARGET romueq_auv2
    OUTPUT_NAME "RomuEq"
    BUNDLE_IDENTIFIER "com.romueq.plugin"
    BUNDLE_VERSION "0.1.0"
    MANUFACTURER_NAME "RomuEq"
    MANUFACTURER_CODE "Romu"
    SUBTYPE_CODE "RmEq"
    INSTRUMENT_TYPE "aufx"
    MACOS_EMBEDDED_CLAP_LOCATION "${ROMUEQ_CLAP_BUNDLE}"
)

