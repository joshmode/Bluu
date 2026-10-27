// tests/driver/avdtp_state_test.cpp
// Host-side tests for the AVDTP transaction, fragmentation, abort, teardown,
// timeout, and backoff state machine.
#include <gtest/gtest.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <vector>

#include "avdtp_state.h"
#include "owb_codec_ids.h"

namespace {

using Bytes = std::vector<unsigned char>;

struct Machine {
    OWB_AVDTP_CONTEXT context{};
    OWB_AVDTP_ACTION action{};

    Machine() { OwbAvdtpStateInit(&context, 0x01); }

    OWB_AVDTP_RESULT receive(const Bytes& bytes) {
        return OwbAvdtpStateHandlePacket(&context, bytes.data(), bytes.size(), &action);
    }

    OWB_AVDTP_RESULT receive(unsigned char message_type, unsigned char signal,
                             const Bytes& payload = {}) {
        Bytes bytes{static_cast<unsigned char>((context.PendingTransactionId << 4u) |
                                              message_type),
                    signal};
        bytes.insert(bytes.end(), payload.begin(), payload.end());
        return receive(bytes);
    }

    void discover() {
        ASSERT_EQ(OwbAvdtpStateBeginDiscover(&context, &action), OwbAvdtpResultCommand);
        ASSERT_EQ(action.SignalId, OWB_AVDTP_MSG_DISCOVER);
        ASSERT_EQ(receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_DISCOVER, {0x04, 0x08}),
                  OwbAvdtpResultCommand);
        ASSERT_EQ(action.SignalId, OWB_AVDTP_MSG_GET_CAPABILITIES);
    }

    void configure() {
        discover();
        const Bytes caps = {0x01, 0x00, 0x07, 0x06, 0x00, 0x00, 0xFF,
                            0xFF, 0x02, 0x35};
        ASSERT_EQ(receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_GET_CAPABILITIES, caps),
                  OwbAvdtpResultCommand);
        ASSERT_EQ(action.SignalId, OWB_AVDTP_MSG_SET_CONFIGURATION);
        ASSERT_EQ(receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_SET_CONFIGURATION),
                  OwbAvdtpResultCommand);
        ASSERT_EQ(action.SignalId, OWB_AVDTP_MSG_OPEN);
        ASSERT_EQ(receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_OPEN),
                  OwbAvdtpResultOpenMedia);
        ASSERT_EQ(OwbAvdtpStateMediaOpened(&context, 1, &action), OwbAvdtpResultCommand);
        ASSERT_EQ(action.SignalId, OWB_AVDTP_MSG_START);
        ASSERT_EQ(receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_START), OwbAvdtpResultNone);
        ASSERT_EQ(context.State, OwbAvdtpStateStreaming);
    }
};

Bytes response(unsigned char txid, unsigned char message_type, unsigned char signal,
               const Bytes& payload = {}, unsigned char packet_type = OWB_AVDTP_PKT_SINGLE) {
    Bytes bytes{static_cast<unsigned char>((txid << 4u) | (packet_type << 2u) |
                                          message_type),
                signal};
    bytes.insert(bytes.end(), payload.begin(), payload.end());
    return bytes;
}

}  // namespace

TEST(AvdtpState, InitSetsHonestDefaults) {
    OWB_AVDTP_CONTEXT context{};
    OwbAvdtpStateInit(&context, 0);
    EXPECT_EQ(context.State, OwbAvdtpStateIdle);
    EXPECT_EQ(context.LocalSeid, 1u);
    EXPECT_EQ(context.ActiveCodecId, OWB_CODEC_SBC);
    EXPECT_EQ(context.PreferredCodecId, OWB_CODEC_SBC);
    EXPECT_EQ(context.PreferredSampleRateHz, 44100u);
    EXPECT_EQ(context.PreferredChannels, 2u);
    EXPECT_STREQ(OwbAvdtpStateName(context.State), "Idle");
}

TEST(AvdtpState, DiscoverUsesIncrementingTransactionIds) {
    Machine machine;
    ASSERT_EQ(OwbAvdtpStateBeginDiscover(&machine.context, &machine.action),
              OwbAvdtpResultCommand);
    EXPECT_EQ(machine.action.TransactionId, 1u);
    EXPECT_TRUE(machine.context.ResponsePending);
    ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_DISCOVER, {0x04, 0x08}),
              OwbAvdtpResultCommand);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_GET_CAPABILITIES);
    EXPECT_EQ(machine.action.TransactionId, 2u);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateConfiguring);
}

TEST(AvdtpState, IgnoresMismatchedTransactionWithoutChangingState) {
    Machine machine;
    ASSERT_EQ(OwbAvdtpStateBeginDiscover(&machine.context, &machine.action),
              OwbAvdtpResultCommand);
    const auto state = machine.context.State;
    const auto pending = machine.context.PendingTransactionId;
    EXPECT_EQ(machine.receive(response(static_cast<unsigned char>(pending + 1u),
                                       OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_DISCOVER,
                                       {0x04, 0x08})),
              OwbAvdtpResultIgnored);
    EXPECT_EQ(machine.context.State, state);
    EXPECT_TRUE(machine.context.ResponsePending);
}

TEST(AvdtpState, RejectFailsClosedAndSchedulesBoundedBackoff) {
    Machine machine;
    ASSERT_EQ(OwbAvdtpStateBeginDiscover(&machine.context, &machine.action),
              OwbAvdtpResultCommand);
    EXPECT_EQ(machine.receive(OWB_AVDTP_MSG_REJECT, OWB_AVDTP_MSG_DISCOVER),
              OwbAvdtpResultRejected);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
    EXPECT_FALSE(machine.context.ResponsePending);
    EXPECT_EQ(machine.context.BackoffTicks, 1u);
    EXPECT_TRUE(machine.action.Reconnect);
    EXPECT_FALSE(OwbAvdtpStateCanReconnect(&machine.context));
    EXPECT_EQ(OwbAvdtpStateTick(&machine.context, &machine.action),
              OwbAvdtpResultIgnored);
    EXPECT_TRUE(OwbAvdtpStateCanReconnect(&machine.context));
}

TEST(AvdtpState, BackoffIsCappedAndClearedByStart) {
    Machine machine;
    for (int i = 0; i < 12; ++i) {
        ASSERT_EQ(OwbAvdtpStateBeginDiscover(&machine.context, &machine.action),
                  OwbAvdtpResultCommand);
        ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_REJECT, OWB_AVDTP_MSG_DISCOVER),
                  OwbAvdtpResultRejected);
    }
    EXPECT_EQ(machine.context.ConsecutiveFailures, 12u);
    EXPECT_EQ(machine.context.BackoffTicks, 8u);

    for (unsigned long i = 0; i < 8; ++i)
        ASSERT_EQ(OwbAvdtpStateTick(&machine.context, &machine.action),
                  OwbAvdtpResultIgnored);
    ASSERT_TRUE(OwbAvdtpStateCanReconnect(&machine.context));
    machine.configure();
    EXPECT_EQ(machine.context.ConsecutiveFailures, 0u);
    EXPECT_EQ(machine.context.BackoffTicks, 0u);
}

TEST(AvdtpState, TimeoutFailsPendingTransactionOnce) {
    Machine machine;
    ASSERT_EQ(OwbAvdtpStateBeginDiscover(&machine.context, &machine.action),
              OwbAvdtpResultCommand);
    ASSERT_EQ(OwbAvdtpStateTick(&machine.context, &machine.action),
              OwbAvdtpResultIgnored);
    ASSERT_EQ(OwbAvdtpStateTick(&machine.context, &machine.action),
              OwbAvdtpResultAborted);
    EXPECT_FALSE(machine.context.ResponsePending);
    EXPECT_EQ(OwbAvdtpStateTimeout(&machine.context, &machine.action),
              OwbAvdtpResultIgnored);
}

TEST(AvdtpState, MediaOpenFailureAborts) {
    Machine machine;
    machine.discover();
    const Bytes caps = {0x01, 0x00, 0x07, 0x06, 0x00, 0x00, 0xFF, 0xFF, 0x02, 0x35};
    ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_GET_CAPABILITIES, caps),
              OwbAvdtpResultCommand);
    ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_SET_CONFIGURATION),
              OwbAvdtpResultCommand);
    ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_OPEN),
              OwbAvdtpResultOpenMedia);
    EXPECT_EQ(OwbAvdtpStateMediaOpened(&machine.context, 0, &machine.action),
              OwbAvdtpResultAborted);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
    EXPECT_TRUE(machine.action.CloseChannels);
    EXPECT_TRUE(machine.action.Reconnect);
}

TEST(AvdtpState, CommandFailureProducesAbortBeforeTeardown) {
    Machine machine;
    machine.configure();
    ASSERT_EQ(OwbAvdtpStateBeginDiscover(&machine.context, &machine.action),
              OwbAvdtpResultIgnored);
    EXPECT_EQ(OwbAvdtpStateCommandFailed(&machine.context, &machine.action),
              OwbAvdtpResultAborted);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
    EXPECT_TRUE(machine.action.HasCommand);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_ABORT);
    EXPECT_EQ(machine.action.PayloadLen, 1u);
    EXPECT_TRUE(machine.action.CloseMedia);
    EXPECT_TRUE(machine.action.CloseChannels);
    EXPECT_TRUE(machine.action.Reconnect);
}

TEST(AvdtpState, ReconfigurationSuspendsThenClosesBeforeDiscovery) {
    Machine machine;
    machine.configure();
    ASSERT_EQ(OwbAvdtpStateSetPreferredCodec(&machine.context, OWB_CODEC_LDAC,
                                             &machine.action),
              OwbAvdtpResultCommand);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_SUSPEND);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateSuspending);
    ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_SUSPEND),
              OwbAvdtpResultCommand);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_CLOSE);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateClosing);
    ASSERT_EQ(machine.receive(OWB_AVDTP_MSG_ACCEPT, OWB_AVDTP_MSG_CLOSE),
              OwbAvdtpResultCommand);
    EXPECT_TRUE(machine.action.CloseMedia);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_DISCOVER);
}

TEST(AvdtpState, FragmentedCapabilitiesAreReassembled) {
    Machine machine;
    machine.discover();
    const auto txid = machine.context.PendingTransactionId;
    const Bytes caps = {0x01, 0x00, 0x07, 0x06, 0x00, 0x00, 0xFF, 0xFF, 0x02, 0x35};
    const Bytes first(caps.begin(), caps.begin() + 4);
    const Bytes second(caps.begin() + 4, caps.end());
    ASSERT_EQ(machine.receive(response(txid, OWB_AVDTP_MSG_ACCEPT,
                                       OWB_AVDTP_MSG_GET_CAPABILITIES, first,
                                       OWB_AVDTP_PKT_START)),
              OwbAvdtpResultIgnored);
    EXPECT_TRUE(machine.context.FragmentActive);
    ASSERT_EQ(machine.receive(response(txid, OWB_AVDTP_MSG_ACCEPT,
                                       OWB_AVDTP_MSG_GET_CAPABILITIES, second,
                                       OWB_AVDTP_PKT_END)),
              OwbAvdtpResultCommand);
    EXPECT_FALSE(machine.context.FragmentActive);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_SET_CONFIGURATION);
}

TEST(AvdtpState, FragmentSequenceMismatchFailsClosed) {
    Machine machine;
    machine.discover();
    const auto txid = machine.context.PendingTransactionId;
    ASSERT_EQ(machine.receive(response(txid, OWB_AVDTP_MSG_ACCEPT,
                                       OWB_AVDTP_MSG_GET_CAPABILITIES, {0x01, 0x00},
                                       OWB_AVDTP_PKT_START)),
              OwbAvdtpResultIgnored);
    EXPECT_EQ(machine.receive(response(static_cast<unsigned char>(txid + 1u),
                                       OWB_AVDTP_MSG_ACCEPT,
                                       OWB_AVDTP_MSG_GET_CAPABILITIES, {0x07},
                                       OWB_AVDTP_PKT_CONTINUE)),
              OwbAvdtpResultProtocolError);
    EXPECT_FALSE(machine.context.FragmentActive);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
}

TEST(AvdtpState, OrphanedContinueFailsClosed) {
    Machine machine;
    machine.discover();
    const auto txid = machine.context.PendingTransactionId;
    EXPECT_EQ(machine.receive(response(txid, OWB_AVDTP_MSG_ACCEPT,
                                       OWB_AVDTP_MSG_GET_CAPABILITIES, {0x07},
                                       OWB_AVDTP_PKT_CONTINUE)),
              OwbAvdtpResultProtocolError);
}

TEST(AvdtpState, OversizedFragmentsFailClosed) {
    Machine machine;
    machine.discover();
    const auto txid = machine.context.PendingTransactionId;
    const Bytes oversized(OWB_AVDTP_SIGNAL_MAX + 1u, 0u);
    EXPECT_EQ(machine.receive(response(txid, OWB_AVDTP_MSG_ACCEPT,
                                       OWB_AVDTP_MSG_GET_CAPABILITIES, oversized,
                                       OWB_AVDTP_PKT_START)),
              OwbAvdtpResultProtocolError);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
}

TEST(AvdtpState, RemoteAbortIsAcknowledgedAndTornDown) {
    Machine machine;
    machine.configure();
    Bytes abort = response(static_cast<unsigned char>(machine.context.TransactionId + 1u),
                           OWB_AVDTP_MSG_CMD,
                           OWB_AVDTP_MSG_ABORT, {0x04});
    EXPECT_EQ(machine.receive(abort), OwbAvdtpResultCommand);
    EXPECT_EQ(machine.action.MessageType, OWB_AVDTP_MSG_ACCEPT);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_ABORT);
    EXPECT_TRUE(machine.action.CloseMedia);
    EXPECT_TRUE(machine.action.CloseChannels);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
}

TEST(AvdtpState, RejectsRemoteStreamCommand) {
    Machine machine;
    Bytes start = response(3u, OWB_AVDTP_MSG_CMD, OWB_AVDTP_MSG_START, {0x04});
    EXPECT_EQ(machine.receive(start), OwbAvdtpResultCommand);
    EXPECT_EQ(machine.action.MessageType, OWB_AVDTP_MSG_REJECT);
    EXPECT_EQ(machine.action.SignalId, OWB_AVDTP_MSG_START);
}

TEST(AvdtpState, PeerDisconnectClearsTransactionAndAllowsReconnect) {
    Machine machine;
    machine.discover();
    OwbAvdtpStatePeerDisconnected(&machine.context, &machine.action);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
    EXPECT_FALSE(machine.context.ResponsePending);
    EXPECT_TRUE(machine.action.Reconnect);
    EXPECT_TRUE(OwbAvdtpStateCanReconnect(&machine.context));
}

TEST(AvdtpState, FormatChangeWhileIdleDoesNotClaimStreaming) {
    Machine machine;
    ASSERT_EQ(OwbAvdtpStateSetPreferredFormat(&machine.context, 48000u, 2u,
                                              &machine.action),
              OwbAvdtpResultIgnored);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
    EXPECT_FALSE(machine.action.HasCommand);
    EXPECT_EQ(machine.context.PreferredSampleRateHz, 48000u);
}

TEST(AvdtpState, TruncatedAndNullPacketsFailClosed) {
    Machine machine;
    EXPECT_EQ(OwbAvdtpStateHandlePacket(&machine.context, nullptr, 0, &machine.action),
              OwbAvdtpResultProtocolError);
    EXPECT_EQ(machine.context.State, OwbAvdtpStateIdle);
}
