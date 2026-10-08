cmake_minimum_required(VERSION 3.22)

# void-plugin-scanner: short-lived contained plugin probe (W06/T22).
# Runs one scan job then exits; hang/crash behaviours are exercised via
# --selftest modes so the supervisor's timeout/quarantine logic is provable.
#
# Build:  cmake -S native/void-plugin-scanner -B build/void-plugin-scanner -G Ninja
#         ninja -C build/void-plugin-scanner

project(void-plugin-scanner VERSION 0.1.0 LANGUAGES C CXX OBJCXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/tracktion_engine/modules/juce
                 ${CMAKE_BINARY_DIR}/_juce)

juce_add_console_app(void-plugin-scanner)
juce_generate_juce_header(void-plugin-scanner)
target_compile_features(void-plugin-scanner PRIVATE cxx_std_20)
target_sources(void-plugin-scanner PRIVATE src/main.cpp)
target_compile_definitions(void-plugin-scanner PRIVATE
    JUCE_PLUGINHOST_AU=1
    JUCE_PLUGINHOST_VST3=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    JUCE_STRICT_REFCOUNTEDPOINTER=1
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_DISPLAY_SPLASH_SCREEN=0)
target_link_libraries(void-plugin-scanner PRIVATE
    juce::juce_audio_processors
    juce::juce_cryptography
    juce::juce_gui_basics
    juce::juce_recommended_warning_flags)
if (APPLE)
    target_link_libraries(void-plugin-scanner PRIVATE "-framework CoreAudio" "-framework CoreMIDI" "-framework AudioUnit" "-framework CoreAudioKit")
endif()
