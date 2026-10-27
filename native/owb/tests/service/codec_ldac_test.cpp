#include <gtest/gtest.h>
#include <vector>
#include "codec_ldac.h"

namespace {
std::vector<int16_t> make_pcm_ldac(int stereo_frames) {
    std::vector<int16_t> buf(static_cast<size_t>(stereo_frames * 2));
    for (int i = 0; i < stereo_frames * 2; ++i)
        buf[i] = static_cast<int16_t>((i % 64) * 512 - 16384);
    return buf;
}
} // namespace

TEST(CodecLdac, NameIsLDAC) {
    owb::CodecLdac codec;
    EXPECT_EQ(codec.name(), "LDAC");
}

TEST(CodecLdac, DefaultQualityIsHQ) {
    owb::CodecLdac codec;
    EXPECT_EQ(codec.get_param("quality"), 0);  // LDACBT_EQMID_HQ
}

TEST(CodecLdac, DefaultFreqIs44100) {
    owb::CodecLdac codec;
    EXPECT_EQ(codec.get_param("freq"), 44100);
}

TEST(CodecLdac, UnknownParamReturnsNullopt) {
    owb::CodecLdac codec;
    EXPECT_EQ(codec.get_param("nonexistent"), std::nullopt);
}

TEST(CodecLdac, SetQualityToSQ) {
    owb::CodecLdac codec;
    EXPECT_TRUE(codec.set_param({"quality", 1}));  // SQ
    EXPECT_EQ(codec.get_param("quality"), 1);
}

TEST(CodecLdac, RejectsInvalidQualityAndRate) {
    owb::CodecLdac codec;
    EXPECT_FALSE(codec.set_param({"quality", 99}));
    EXPECT_FALSE(codec.set_param({"freq", 32000}));
    EXPECT_EQ(codec.get_param("quality"), 0);
    EXPECT_EQ(codec.get_param("freq"), 44100);
}

TEST(CodecLdac, EncodeProducesOutput) {
    owb::CodecLdac codec;
    // The LDAC encoder accumulates nfrm_in_pkt frames (typically 4-6 at MTU=895)
    // before flushing a transport packet. Feed up to 16 x 128-sample blocks until
    // the encoder emits output.
    std::vector<uint8_t> out(4096);
    std::ptrdiff_t n = -1;
    for (int attempt = 0; attempt < 16 && n <= 0; ++attempt)
        n = codec.encode(make_pcm_ldac(128), out);
    EXPECT_GT(n, 0) << "LDAC encode must produce output within 16 x 128-sample calls";
}

TEST(CodecLdac, EncodeReturnsZeroWhileBufferingWithoutClaimingOutputSize) {
    owb::CodecLdac codec;
    std::vector<uint8_t> out(4096);
    const auto pcm = make_pcm_ldac(128);
    std::ptrdiff_t total = 0;
    bool saw_zero = false;
    for (int attempt = 0; attempt < 16; ++attempt) {
        const auto n = codec.encode(pcm, out);
        EXPECT_GE(n, 0);
        if (n == 0) saw_zero = true;
        total += n;
    }
    EXPECT_TRUE(saw_zero);
    EXPECT_GT(total, 0);
}
