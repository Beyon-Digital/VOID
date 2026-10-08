cmake_minimum_required(VERSION 3.22)

# void-engine: Tracktion Engine worker process for VOID.
# Build:  cmake -S native/void-engine -B build/void-engine -G Ninja
#         ninja -C build/void-engine

project(void-engine VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# --- third-party -------------------------------------------------------------
# TE brings JUCE along as modules/juce (pinned nested submodule).
add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/tracktion_engine/modules/juce
                 ${CMAKE_BINARY_DIR}/_juce)
add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/tracktion_engine/modules
                 ${CMAKE_BINARY_DIR}/_tracktion)

# flatbuffers is headers-only for the C++ runtime.
add_library(flatbuffers_rt INTERFACE)
target_include_directories(flatbuffers_rt INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/flatbuffers/include)

# --- void-engine -------------------------------------------------------------
juce_add_console_app(void-engine)
juce_generate_juce_header(void-engine)

target_compile_features(void-engine PRIVATE cxx_std_20)

target_sources(void-engine PRIVATE
    src/main.cpp
    src/bridge/UdsChannel.cpp
    src/session/EngineSession.cpp
    src/session/Ops.cpp
    src/persistence/SaveBridge.cpp
    src/plugins/PluginHost.cpp
    src/audio/DeviceBridge.cpp
    src/recording/TakeJournal.cpp
    src/recording/MidiCapture.cpp
    src/recording/RecordingManager.cpp)

target_include_directories(void-engine PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/../../protocol/generated/cpp)

target_compile_definitions(void-engine PRIVATE
    JUCE_PLUGINHOST_AU=1
    JUCE_PLUGINHOST_VST3=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    JUCE_STRICT_REFCOUNTEDPOINTER=1
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_DISPLAY_SPLASH_SCREEN=0
    TRACKTION_ENABLE_TIMESTRETCH_SOUNDTOUCH=0
    TRACKTION_ENABLE_TIMESTRETCH_RUBBERBAND=0
    TRACKTION_BUILD_RUBBERBAND=0
    TRACKTION_LOG_DEVICES=0
    TRACKTION_UNIT_TESTS=0)

target_link_libraries(void-engine PRIVATE
    tracktion::tracktion_core
    tracktion::tracktion_engine
    tracktion::tracktion_graph
    juce::juce_audio_devices
    juce::juce_audio_processors
    juce::juce_audio_utils
    juce::juce_gui_basics
    juce::juce_cryptography
    flatbuffers_rt
    juce::juce_recommended_warning_flags)

if (APPLE)
    set_target_properties(void-engine PROPERTIES
        C_VISIBILITY_PRESET hidden
        CXX_VISIBILITY_PRESET hidden)
    target_compile_options(void-engine PRIVATE -fno-aligned-allocation)
    target_link_libraries(void-engine PRIVATE "-framework CoreAudio" "-framework CoreMIDI")
endif()

# --- T14 render fixture -------------------------------------------------------
juce_add_console_app(void-render-fixture)
juce_generate_juce_header(void-render-fixture)
target_compile_features(void-render-fixture PRIVATE cxx_std_20)
target_sources(void-render-fixture PRIVATE tests/render_fixture.cpp)
target_compile_definitions(void-render-fixture PRIVATE
    JUCE_PLUGINHOST_AU=1 JUCE_PLUGINHOST_VST3=1
    JUCE_USE_CURL=0 JUCE_WEB_BROWSER=0
    JUCE_STRICT_REFCOUNTEDPOINTER=1
    JUCE_MODAL_LOOPS_PERMITTED=1
    TRACKTION_ENABLE_TIMESTRETCH_SOUNDTOUCH=0
    TRACKTION_ENABLE_TIMESTRETCH_RUBBERBAND=0
    TRACKTION_BUILD_RUBBERBAND=0
    TRACKTION_LOG_DEVICES=0 TRACKTION_UNIT_TESTS=0)
target_link_libraries(void-render-fixture PRIVATE
    tracktion::tracktion_core tracktion::tracktion_engine tracktion::tracktion_graph
    juce::juce_audio_devices juce::juce_audio_processors juce::juce_audio_utils
    juce::juce_cryptography
    flatbuffers_rt juce::juce_recommended_warning_flags)
if (APPLE)
    target_compile_options(void-render-fixture PRIVATE -fno-aligned-allocation)
endif()

# --- W08 recording fixture ---------------------------------------------------
# Drives EngineSession + RecordingManager in-process through the hosted device
# (test_utilities::EnginePlayer) for deterministic take evidence headless.
juce_add_console_app(void-recording-fixture)
juce_generate_juce_header(void-recording-fixture)
target_compile_features(void-recording-fixture PRIVATE cxx_std_20)
target_sources(void-recording-fixture PRIVATE
    tests/recording_fixture.cpp
    src/session/EngineSession.cpp
    src/session/Ops.cpp
    src/persistence/SaveBridge.cpp
    src/plugins/PluginHost.cpp
    src/audio/DeviceBridge.cpp
    src/recording/TakeJournal.cpp
    src/recording/MidiCapture.cpp
    src/recording/RecordingManager.cpp)
target_include_directories(void-recording-fixture PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/../../protocol/generated/cpp)
target_compile_definitions(void-recording-fixture PRIVATE
    JUCE_PLUGINHOST_AU=1 JUCE_PLUGINHOST_VST3=1
    JUCE_USE_CURL=0 JUCE_WEB_BROWSER=0
    JUCE_STRICT_REFCOUNTEDPOINTER=1
    JUCE_MODAL_LOOPS_PERMITTED=1
    TRACKTION_ENABLE_TIMESTRETCH_SOUNDTOUCH=0
    TRACKTION_ENABLE_TIMESTRETCH_RUBBERBAND=0
    TRACKTION_BUILD_RUBBERBAND=0
    TRACKTION_LOG_DEVICES=0 TRACKTION_UNIT_TESTS=0)
target_link_libraries(void-recording-fixture PRIVATE
    tracktion::tracktion_core tracktion::tracktion_engine tracktion::tracktion_graph
    juce::juce_audio_devices juce::juce_audio_processors juce::juce_audio_utils
    juce::juce_gui_basics juce::juce_cryptography
    flatbuffers_rt juce::juce_recommended_warning_flags)
if (APPLE)
    target_compile_options(void-recording-fixture PRIVATE -fno-aligned-allocation)
    target_link_libraries(void-recording-fixture PRIVATE "-framework CoreAudio" "-framework CoreMIDI")
endif()
