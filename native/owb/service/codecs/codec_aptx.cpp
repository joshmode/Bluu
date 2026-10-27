// service/codecs/codec_aptx.cpp
//
// The encoder is the codec core from FFmpeg's LGPL-2.1-or-later aptX
// implementation (see third-party/ffmpeg-aptx). The A2DP packetizer remains in
// the driver. ICodec::encode() receives int16 PCM and expands it to the 24-bit
// signed samples expected by the FFmpeg predictor/quantizer.

#include "codec_aptx.h"

extern "C" {
#include "aptx_core.h"
}

#include <cstring>

namespace owb {

static constexpr size_t kSamplesPerBlock = 4;
static constexpr size_t kChannels = 2;

static constexpr int64_t bits_per_stereo_sample(bool hd) noexcept {
    const int64_t block_bytes = hd ? 6 : 4;
    return block_bytes * 8 / static_cast<int64_t>(kSamplesPerBlock);
}

// ── CodecAptx ────────────────────────────────────────────────────────────────

CodecAptx::CodecAptx(bool hd)
    : ctx_(std::make_unique<AptXContext>()), hd_(hd), freq_(44100) {
    std::memset(ctx_.get(), 0, sizeof(AptXContext));
    ff_aptx_init(ctx_.get(), hd ? 1 : 0);
}

CodecAptx::~CodecAptx() = default;

std::string_view CodecAptx::name() const noexcept {
    return hd_ ? "aptX-HD" : "aptX";
}

std::ptrdiff_t CodecAptx::encode(std::span<const int16_t> input,
                                  std::span<uint8_t>       output) {
    if (!ctx_ || output.empty() || input.empty()) return -1;

    const size_t min_samples = kSamplesPerBlock * kChannels;
    if (input.size() < min_samples) return -1;
    // Do not silently drop a partial block. The pipeline consumes exactly
    // input_frame_samples(), but this guard keeps direct callers honest.
    if (input.size() % min_samples != 0u) return -1;

    const size_t num_blocks = input.size() / min_samples;
    const size_t bytes_per_block = hd_ ? 6u : 4u;

    std::ptrdiff_t total_written = 0;

    for (size_t b = 0; b < num_blocks; ++b) {
        if (static_cast<size_t>(total_written) + bytes_per_block > output.size()) break;

        int32_t samples[NB_CHANNELS][4] = {};
        for (size_t channel = 0; channel < kChannels; ++channel) {
            for (size_t sample = 0; sample < kSamplesPerBlock; ++sample) {
                const int16_t pcm = input[b * min_samples + sample * kChannels + channel];
                samples[channel][sample] = static_cast<int32_t>(pcm) << 8;
            }
        }
        ff_aptx_encode_block(
            ctx_.get(), samples,
            output.data() + static_cast<size_t>(total_written));
        total_written += static_cast<std::ptrdiff_t>(bytes_per_block);
    }

    return total_written > 0 ? total_written : -1;
}

bool CodecAptx::set_param(CodecParam p) {
    if (p.key == "freq") {
        if (p.value != 44100 && p.value != 48000) return false;
        freq_ = static_cast<int>(p.value);
        return true;
    }
    return false;
}

std::optional<int64_t> CodecAptx::get_param(std::string_view key) const {
    if (key == "freq") return freq_;
    if (key == "hd")   return hd_ ? 1 : 0;
    if (key == "bitrate") return static_cast<int64_t>(freq_) * bits_per_stereo_sample(hd_);
    return std::nullopt;
}

} // namespace owb
