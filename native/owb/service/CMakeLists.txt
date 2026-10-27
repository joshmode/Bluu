# ── libsbc static library (BlueZ SBC codec, LGPL 2.1) ───────────────────────
add_library(owb_sbc STATIC
    ${CMAKE_SOURCE_DIR}/third-party/libsbc/sbc/sbc.c
    ${CMAKE_SOURCE_DIR}/third-party/libsbc/sbc/sbc_primitives.c
)
target_include_directories(owb_sbc PUBLIC
    ${CMAKE_SOURCE_DIR}/third-party/libsbc          # for <sbc/sbc.h>
    ${CMAKE_SOURCE_DIR}/third-party/libsbc/sbc      # for sbc_private.h etc.
    ${CMAKE_SOURCE_DIR}/third-party/libsbc-compat   # for sbc_compat.h
)
target_compile_options(owb_sbc PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W0 /utf-8>
)
# MSVC: force-include sbc_compat.h before every sbc source file
target_compile_options(owb_sbc PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/FIsbc_compat.h>
)

# ── SBC codec C++ wrapper (ICodec implementation) ────────────────────────────
add_library(owb_codec_sbc STATIC
    codecs/codec_sbc.cpp
)
target_include_directories(owb_codec_sbc PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
)
target_link_libraries(owb_codec_sbc PUBLIC owb_sbc)
target_compile_options(owb_codec_sbc PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8 /FIsbc_compat.h>
)

# ── FFmpeg aptX core (LGPL-2.1-or-later) ────────────────────────────────────
# Only the codec DSP tables and block encoder are vendored; FFmpeg's framework
# and command-line components are not part of Bluu.
add_library(owb_aptx STATIC
    ${CMAKE_SOURCE_DIR}/third-party/ffmpeg-aptx/aptx_core.c
)
target_include_directories(owb_aptx PUBLIC
    ${CMAKE_SOURCE_DIR}/third-party/ffmpeg-aptx
)
target_compile_options(owb_aptx PRIVATE
    $<$<C_COMPILER_ID:MSVC>:/W3 /utf-8>
)

# ── aptX codec C++ wrapper ─────────────────────────────────────────────────────
add_library(owb_codec_aptx STATIC
    codecs/codec_aptx.cpp
)
target_include_directories(owb_codec_aptx PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
)
target_link_libraries(owb_codec_aptx PUBLIC owb_aptx)
target_compile_options(owb_codec_aptx PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)

# ── libldac static library (Sony LDAC encoder, Apache 2.0) ──────────────────
add_library(owb_ldac STATIC
    ${CMAKE_SOURCE_DIR}/third-party/libldac/src/ldacBT.c
    ${CMAKE_SOURCE_DIR}/third-party/libldac/src/ldaclib.c
)
target_compile_definitions(owb_ldac PRIVATE _ENCODE_ONLY)
target_include_directories(owb_ldac PUBLIC
    ${CMAKE_SOURCE_DIR}/third-party/libldac/inc
    ${CMAKE_SOURCE_DIR}/third-party/libldac/src
    ${CMAKE_SOURCE_DIR}/third-party/libldac-compat
)
target_compile_options(owb_ldac PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W0 /utf-8 /FIldac_compat.h>
)

# ── LDAC codec C++ wrapper ────────────────────────────────────────────────────
add_library(owb_codec_ldac STATIC
    codecs/codec_ldac.cpp
)
target_include_directories(owb_codec_ldac PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
)
target_link_libraries(owb_codec_ldac PUBLIC owb_ldac)
target_compile_options(owb_codec_ldac PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)

# ── AAC codec wrapper (Windows Media Foundation built-in encoder) ─────────────
add_library(owb_codec_aac STATIC
    codecs/codec_aac.cpp
    codecs/latm.cpp
)
target_include_directories(owb_codec_aac PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
)
target_compile_options(owb_codec_aac PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)

# ── Codec factory (selects codec by OWB_CODEC_* ID) ─────────────────────────
add_library(owb_codec_factory STATIC
    codecs/codec_factory.cpp
)
target_include_directories(owb_codec_factory PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
    ${CMAKE_SOURCE_DIR}/driver          # for owb_ioctl.h
)
target_link_libraries(owb_codec_factory PUBLIC
    owb_codec_sbc owb_codec_aptx owb_codec_ldac owb_codec_aac
)
target_compile_options(owb_codec_factory PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)

# ── Logger de archivo (compartido por todos los componentes del servicio) ────
add_library(owb_log STATIC
    src/owb_log.cpp
)
target_include_directories(owb_log PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_compile_options(owb_log PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)

# ── HFP Guard static library ─────────────────────────────────────────────────
add_library(owb_hfp_guard STATIC
    src/hfp_guard.cpp
)
target_include_directories(owb_hfp_guard PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_compile_options(owb_hfp_guard PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)
target_link_libraries(owb_hfp_guard PUBLIC
    owb_log
    ole32 oleaut32 uuid Mmdevapi
)

# ── IPC server static library ─────────────────────────────────────────────────
add_library(owb_ipc_server STATIC
    src/ipc_server.cpp
)
target_include_directories(owb_ipc_server PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_SOURCE_DIR}/driver        # for owb_ioctl.h (via a2dp_stream.h)
)
target_compile_options(owb_ipc_server PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)
target_link_libraries(owb_ipc_server PUBLIC
    owb_log
    advapi32   # ConvertStringSecurityDescriptorToSecurityDescriptorW
)

# ── A2DP stream (DeviceIoControl to kernel driver) ────────────────────────────
add_library(owb_a2dp_stream STATIC
    src/a2dp_stream.cpp
)
target_include_directories(owb_a2dp_stream PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_SOURCE_DIR}/driver        # for owb_ioctl.h
)
target_compile_options(owb_a2dp_stream PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)
target_link_libraries(owb_a2dp_stream PUBLIC
    owb_log
)

# ── Stream pipeline (capture → AI → codec → driver orchestrator) ─────────────
add_library(owb_stream_pipeline STATIC
    src/stream_pipeline.cpp
)
target_include_directories(owb_stream_pipeline PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
)
target_compile_options(owb_stream_pipeline PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)
target_link_libraries(owb_stream_pipeline PUBLIC
    owb_codec_factory owb_a2dp_stream
)


# ── Service executable ─────────────────────────────────────────────────────
add_executable(owb_service WIN32
    src/main.cpp
)
target_include_directories(owb_service PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/codecs
    ${CMAKE_SOURCE_DIR}/driver
)
target_link_libraries(owb_service PRIVATE
    owb_log
    owb_hfp_guard
    owb_ipc_server
    owb_a2dp_stream
    owb_stream_pipeline
    owb_codec_factory
    ole32 oleaut32 uuid Mmdevapi mfplat mfuuid
)
target_compile_options(owb_service PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /utf-8>
)
