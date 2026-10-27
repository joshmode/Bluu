// driver/src/avdtp_state.h
// WDK-free AVDTP signaling state machine, shared by the driver and host tests.
#pragma once
#include <stddef.h>

#include "avdtp_packets.h"
#include "owb_codec_ids.h"

#ifdef __cplusplus
extern "C" {
#endif

// AVDTP signal identifiers (A2DP 1.3 section 8.5).
#define OWB_AVDTP_MSG_DISCOVER          0x01u
#define OWB_AVDTP_MSG_GET_CAPABILITIES  0x02u
#define OWB_AVDTP_MSG_SET_CONFIGURATION 0x03u
#define OWB_AVDTP_MSG_OPEN              0x06u
#define OWB_AVDTP_MSG_START             0x07u
#define OWB_AVDTP_MSG_CLOSE             0x08u
#define OWB_AVDTP_MSG_SUSPEND           0x09u
#define OWB_AVDTP_MSG_ABORT             0x0Au

// Packet type and message type bit fields in header byte 0.
#define OWB_AVDTP_PKT_SINGLE    0x00u
#define OWB_AVDTP_PKT_START     0x01u
#define OWB_AVDTP_PKT_CONTINUE  0x02u
#define OWB_AVDTP_PKT_END       0x03u
#define OWB_AVDTP_MSG_CMD         0x00u
#define OWB_AVDTP_MSG_ACCEPT      0x02u
#define OWB_AVDTP_MSG_REJECT      0x03u

// A reassembled AVDTP signaling message above this size is rejected. The
// L2CAP signaling MTU is normally 672 bytes; the extra headroom permits a
// two-fragment response without unbounded buffering in the driver.
#define OWB_AVDTP_SIGNAL_MAX 1024u

typedef enum _OWB_AVDTP_STATE {
    OwbAvdtpStateIdle = 0,
    OwbAvdtpStateConnecting = 1,
    OwbAvdtpStateDiscovering = 2,
    OwbAvdtpStateConfiguring = 3,
    OwbAvdtpStateConfigured = 4,
    OwbAvdtpStateOpen = 5,
    OwbAvdtpStateStreaming = 6,
    OwbAvdtpStateSuspending = 7,
    OwbAvdtpStateClosing = 8,
} OWB_AVDTP_STATE;

typedef enum _OWB_AVDTP_RESULT {
    OwbAvdtpResultNone = 0,
    OwbAvdtpResultCommand = 1,
    OwbAvdtpResultOpenMedia = 2,
    OwbAvdtpResultIgnored = 3,
    OwbAvdtpResultAborted = 4,
    OwbAvdtpResultRejected = 5,
    OwbAvdtpResultProtocolError = 6,
} OWB_AVDTP_RESULT;

typedef struct _OWB_AVDTP_ACTION {
    OWB_AVDTP_RESULT Result;
    int              HasCommand;
    unsigned char    MessageType;
    unsigned char    SignalId;
    unsigned char    TransactionId;
    unsigned short   PayloadLen;
    int              OpenMedia;
    int              CloseMedia;
    int              CloseChannels;
    int              Reconnect;
    unsigned char    Payload[OWB_AVDTP_SET_CONFIGURATION_MAX_LEN];
} OWB_AVDTP_ACTION, *POWB_AVDTP_ACTION;

typedef struct _OWB_AVDTP_CONTEXT {
    OWB_AVDTP_STATE State;
    unsigned short  SignalingCid;
    unsigned short  MediaCid;
    unsigned char   RemoteSeid;
    unsigned char   LocalSeid;
    unsigned char   TransactionId;
    unsigned char   PendingTransactionId;
    unsigned char   PendingSignalId;
    int             ResponsePending;
    unsigned long   ActiveCodecId;
    unsigned long   PendingCodecId;
    unsigned long   PreferredCodecId;
    unsigned long   PreferredSampleRateHz;
    unsigned long   PreferredChannels;
    unsigned long   PendingTimeoutTicks;
    unsigned long   ConsecutiveFailures;
    unsigned long   BackoffTicks;

    // Bounded response reassembly. A fragment sequence is kept separate from
    // ResponsePending so malformed input can fail closed without ambiguity.
    int             FragmentActive;
    unsigned char   FragmentTransactionId;
    unsigned char   FragmentSignalId;
    unsigned char   FragmentMessageType;
    unsigned short  FragmentLength;
    unsigned char   FragmentBuffer[OWB_AVDTP_SIGNAL_MAX];
} OWB_AVDTP_CONTEXT, *POWB_AVDTP_CONTEXT;

void OwbAvdtpStateInit(POWB_AVDTP_CONTEXT Context, unsigned char local_seid);

// Starts discovery. Returns a COMMAND action with DISCOVER.
OWB_AVDTP_RESULT OwbAvdtpStateBeginDiscover(
    POWB_AVDTP_CONTEXT Context,
    POWB_AVDTP_ACTION Action);

// Processes one complete or fragmented signaling packet.
OWB_AVDTP_RESULT OwbAvdtpStateHandlePacket(
    POWB_AVDTP_CONTEXT Context,
    const unsigned char* Data,
    size_t Length,
    POWB_AVDTP_ACTION Action);

// Called after the media L2CAP channel opens (or fails to open) following the
// AVDTP OPEN response. On success this produces the START command.
OWB_AVDTP_RESULT OwbAvdtpStateMediaOpened(
    POWB_AVDTP_CONTEXT Context,
    int MediaOpenSucceeded,
    POWB_AVDTP_ACTION Action);

// A command could not be submitted to L2CAP. Resets the transaction and
// schedules bounded reconnect backoff.
OWB_AVDTP_RESULT OwbAvdtpStateCommandFailed(
    POWB_AVDTP_CONTEXT Context,
    POWB_AVDTP_ACTION Action);

// A pending response timed out.
OWB_AVDTP_RESULT OwbAvdtpStateTimeout(
    POWB_AVDTP_CONTEXT Context,
    POWB_AVDTP_ACTION Action);

// Advances response-timeout and reconnect-backoff timers. Call periodically
// (the driver's reconnect timer uses a 3-second cadence).
OWB_AVDTP_RESULT OwbAvdtpStateTick(
    POWB_AVDTP_CONTEXT Context,
    POWB_AVDTP_ACTION Action);

// Updates the desired codec. While streaming, starts SUSPEND -> CLOSE ->
// renegotiation; otherwise the preference applies to the next setup.
OWB_AVDTP_RESULT OwbAvdtpStateSetPreferredCodec(
    POWB_AVDTP_CONTEXT Context,
    unsigned long CodecId,
    POWB_AVDTP_ACTION Action);

// Updates the requested PCM format used to build SET_CONFIGURATION.
OWB_AVDTP_RESULT OwbAvdtpStateSetPreferredFormat(
    POWB_AVDTP_CONTEXT Context,
    unsigned long SampleRateHz,
    unsigned long Channels,
    POWB_AVDTP_ACTION Action);

// The remote L2CAP channel disconnected.
void OwbAvdtpStatePeerDisconnected(
    POWB_AVDTP_CONTEXT Context,
    POWB_AVDTP_ACTION Action);

// True when a fresh signaling channel may be opened.
int OwbAvdtpStateCanReconnect(const OWB_AVDTP_CONTEXT* Context);

// True only when the codec is the negotiated active codec and media is streaming.
int OwbAvdtpStateCodecAllowed(
    const OWB_AVDTP_CONTEXT* Context,
    unsigned long CodecId);

const char* OwbAvdtpStateName(OWB_AVDTP_STATE State);

#ifdef __cplusplus
}
#endif
