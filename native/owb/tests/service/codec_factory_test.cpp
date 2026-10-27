// tests/service/codec_factory_test.cpp
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <winioctl.h>    // CTL_CODE — must precede owb_ioctl.h (via codec_factory.h)
#include <gtest/gtest.h>
#include "codec_factory.h"

// OWB_CODEC_* constants are in driver/owb_ioctl.h — included via codec_factory.h
TEST(CodecFactory, CreateSbc_NamedSBC) {
    auto codec = owb::CodecFactory::create(OWB_CODEC_SBC);
    ASSERT_NE(codec, nullptr);
    EXPECT_EQ(codec->name(), "SBC");
}

TEST(CodecFactory, CreateLdac_NamedLDAC) {
    auto codec = owb::CodecFactory::create(OWB_CODEC_LDAC);
    ASSERT_NE(codec, nullptr);
    EXPECT_EQ(codec->name(), "LDAC");
}

TEST(CodecFactory, CreateAptx_NamedAptX) {
    auto codec = owb::CodecFactory::create(OWB_CODEC_APTX);
    ASSERT_NE(codec, nullptr);
    EXPECT_EQ(codec->name(), "aptX");
}

TEST(CodecFactory, CreateAptxHd_NamedAptXHD) {
    auto codec = owb::CodecFactory::create(OWB_CODEC_APTXHD);
    ASSERT_NE(codec, nullptr);
    EXPECT_EQ(codec->name(), "aptX-HD");
}

TEST(CodecFactory, CreateAac_NamedAAC) {
    auto codec = owb::CodecFactory::create(OWB_CODEC_AAC);
    ASSERT_NE(codec, nullptr);
    EXPECT_EQ(codec->name(), "AAC");
}

TEST(CodecFactory, CreateUnknownReturnsNullInsteadOfMisencodingAsSbc) {
    auto codec = owb::CodecFactory::create(99u);
    EXPECT_EQ(codec, nullptr);
}

TEST(CodecFactory, UnsupportedLeAudioAndAdaptiveAreNotPretended) {
    EXPECT_EQ(owb::CodecFactory::create(OWB_CODEC_LC3), nullptr);
    EXPECT_EQ(owb::CodecFactory::create(OWB_CODEC_APTX_ADAPTIVE), nullptr);
}
