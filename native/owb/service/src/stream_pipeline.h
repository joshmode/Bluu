// service/src/stream_pipeline.h
#pragma once
#include <cstdint>
#include <memory>
#include <string>
#include <string_view>

#include "codec_controller.h"

namespace owb { class A2dpStream; class IAudioSource; }

namespace owb {

// StreamPipeline — the missing runtime orchestrator.
//
// On its own thread it pulls PCM from an IAudioSource, encodes with the active
// codec, and pushes each encoded frame to the kernel driver via A2dpStream.
//
// The active codec is swappable at runtime via set_codec_id() — the IPC server
// calls it when the GUI requests a different codec.
class StreamPipeline final : public ICodecController {
public:
    StreamPipeline(IAudioSource* source, A2dpStream* sink);
    ~StreamPipeline() override;

    StreamPipeline(const StreamPipeline&)            = delete;
    StreamPipeline& operator=(const StreamPipeline&) = delete;

    // Spawn the streaming thread. Returns false if the source cannot start.
    bool start();

    // Stop the thread and release the codec. Safe to call when not started.
    void stop();

    // ICodecController
    void        set_codec_id(uint32_t codec_id) override;
    bool        set_codec_param(std::string_view key, int64_t value) override;
    std::optional<int64_t> get_codec_param(std::string_view key) const override;
    uint32_t    codec_id() const noexcept       override;
    std::string codec_name() const              override;
    bool        is_streaming() const noexcept   override;
    uint32_t    bitrate() const                 override;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace owb
