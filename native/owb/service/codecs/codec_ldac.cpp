// service/codecs/codec_ldac.cpp
#include "codec_ldac.h"
#include <ldacBT.h>

namespace owb {

CodecLdac::CodecLdac() {
    handle_ = ldacBT_get_handle();
    reinit();
}

CodecLdac::~CodecLdac() {
    if (handle_) {
        ldacBT_close_handle(handle_);
        ldacBT_free_handle(handle_);
    }
}

void CodecLdac::reinit() {
    if (!handle_) return;
    ldacBT_close_handle(handle_);
    initialized_ = ldacBT_init_handle_encode(
        handle_, kMtu, quality_,
        LDACBT_CHANNEL_MODE_STEREO,
        LDACBT_SMPL_FMT_S16, freq_
    ) == 0;
}

std::string_view CodecLdac::name() const noexcept { return "LDAC"; }

// LDAC encodes one LSU of 128 samples per channel per call → 256 interleaved.
int CodecLdac::input_frame_samples() const noexcept { return LDACBT_ENC_LSU * 2; }

std::ptrdiff_t CodecLdac::encode(std::span<const int16_t> input,
                                  std::span<uint8_t>       output) {
    if (!handle_ || !initialized_ || output.empty() || input.empty()) return -1;

    // ldacBT_encode consumes one fixed 128-sample LSU per call. Feed exactly
    // that many stereo samples; the streaming orchestrator owns buffering.
    constexpr size_t kStereoLsuSamples = static_cast<size_t>(LDACBT_ENC_LSU) * 2;
    if (input.size() < kStereoLsuSamples) return -1;

    int frame_num = 0;
    int pcm_used = 0;
    int stream_used = static_cast<int>(output.size());
    void* pcm_ptr = const_cast<int16_t*>(input.data());
    const int result = ldacBT_encode(
        handle_,
        pcm_ptr, &pcm_used,
        output.data(), &stream_used,
        &frame_num
    );
    if (result != 0 || pcm_used != static_cast<int>(kStereoLsuSamples * sizeof(int16_t)))
        return -1;
    if (stream_used <= 0) return 0;       // still buffering — not an error
    return static_cast<std::ptrdiff_t>(stream_used);
}

bool CodecLdac::set_param(CodecParam p) {
    if (p.key == "quality") {
        if (p.value < LDACBT_EQMID_HQ || p.value > LDACBT_EQMID_MQ) return false;
        const int previous = quality_;
        quality_ = static_cast<int>(p.value);
        if (ldacBT_set_eqmid(handle_, quality_) != 0) {
            quality_ = previous;
            return false;
        }
        return true;
    }
    if (p.key == "freq") {
        if (p.value != 44100 && p.value != 48000) return false;
        const int previous = freq_;
        freq_ = static_cast<int>(p.value);
        reinit();
        if (!initialized_) {
            freq_ = previous;
            reinit();
        }
        return initialized_;
    }
    return false;
}

std::optional<int64_t> CodecLdac::get_param(std::string_view key) const {
    if (key == "quality") return quality_;
    if (key == "freq")    return freq_;
    if (key == "bitrate") return ldacBT_get_bitrate(handle_);
    return std::nullopt;
}

} // namespace owb
