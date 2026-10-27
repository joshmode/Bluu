// driver/src/avdtp_state.c
// WDK-free AVDTP signaling state machine. Keep this file free of Windows APIs
// so every transition can be exercised by the host test suite.
#include "avdtp_state.h"

static void zero_bytes(void* destination, size_t length) {
    unsigned char* bytes = (unsigned char*)destination;
    for (size_t i = 0u; i < length; ++i) bytes[i] = 0u;
}

static void copy_bytes(unsigned char* destination, const unsigned char* source,
                       size_t length) {
    for (size_t i = 0u; i < length; ++i) destination[i] = source[i];
}

static void action_init(OWB_AVDTP_ACTION* action) {
    zero_bytes(action, sizeof(*action));
    action->Result = OwbAvdtpResultNone;
}

static void clear_fragment(POWB_AVDTP_CONTEXT context) {
    context->FragmentActive = 0;
    context->FragmentLength = 0u;
    context->FragmentTransactionId = 0u;
    context->FragmentSignalId = 0u;
    context->FragmentMessageType = 0u;
}

static void clear_pending(POWB_AVDTP_CONTEXT context) {
    context->ResponsePending = 0;
    context->PendingTransactionId = 0u;
    context->PendingSignalId = 0u;
    context->PendingTimeoutTicks = 0u;
}

static void queue_abort(POWB_AVDTP_CONTEXT context, OWB_AVDTP_ACTION* action) {
    if (context->RemoteSeid == OWB_AVDTP_SEID_NONE) return;
    if (context->State == OwbAvdtpStateIdle ||
        context->State == OwbAvdtpStateConnecting ||
        context->State == OwbAvdtpStateDiscovering) {
        return;
    }
    const unsigned char payload =
        (unsigned char)(((unsigned int)context->RemoteSeid << 2u) & 0xFCu);
    action->HasCommand = 1;
    action->MessageType = OWB_AVDTP_MSG_CMD;
    action->SignalId = OWB_AVDTP_MSG_ABORT;
    action->TransactionId = (unsigned char)((context->TransactionId + 1u) & 0x0Fu);
    action->PayloadLen = 1u;
    action->Payload[0] = payload;
}

static OWB_AVDTP_RESULT enter_failure(POWB_AVDTP_CONTEXT context,
                                      OWB_AVDTP_ACTION* action,
                                      OWB_AVDTP_RESULT result) {
    action_init(action);
    queue_abort(context, action);
    clear_pending(context);
    clear_fragment(context);
    context->State = OwbAvdtpStateIdle;
    context->ActiveCodecId = OWB_CODEC_SBC;
    context->PendingCodecId = OWB_CODEC_SBC;
    if (context->ConsecutiveFailures < 16u) context->ConsecutiveFailures++;
    const unsigned long shift =
        context->ConsecutiveFailures > 4u ? 3u : context->ConsecutiveFailures - 1u;
    context->BackoffTicks = 1u << shift;
    if (context->BackoffTicks > 8u) context->BackoffTicks = 8u;
    action->Result = result;
    action->CloseChannels = 1;
    action->CloseMedia = 1;
    action->Reconnect = 1;
    return result;
}

static void clear_failure(POWB_AVDTP_CONTEXT context) {
    context->ConsecutiveFailures = 0u;
    context->BackoffTicks = 0u;
}

static void queue_command(POWB_AVDTP_CONTEXT context, OWB_AVDTP_ACTION* action,
                          unsigned char signal_id, const unsigned char* payload,
                          unsigned short payload_length) {
    action_init(action);
    context->TransactionId = (unsigned char)((context->TransactionId + 1u) & 0x0Fu);
    context->PendingTransactionId = context->TransactionId;
    context->PendingSignalId = signal_id;
    context->ResponsePending = 1;
    context->PendingTimeoutTicks = 2u;
    action->Result = OwbAvdtpResultCommand;
    action->HasCommand = 1;
    action->MessageType = OWB_AVDTP_MSG_CMD;
    action->SignalId = signal_id;
    action->TransactionId = context->TransactionId;
    if (payload_length > sizeof(action->Payload))
        payload_length = (unsigned short)sizeof(action->Payload);
    action->PayloadLen = payload_length;
    if (payload_length > 0u) copy_bytes(action->Payload, payload, payload_length);
}

static void queue_response(OWB_AVDTP_ACTION* action, unsigned char message_type,
                           unsigned char signal_id, unsigned char transaction_id) {
    action_init(action);
    action->Result = OwbAvdtpResultCommand;
    action->HasCommand = 1;
    action->MessageType = message_type;
    action->SignalId = signal_id;
    action->TransactionId = transaction_id;
}

static unsigned char seid_payload(POWB_AVDTP_CONTEXT context) {
    return (unsigned char)(((unsigned int)context->RemoteSeid << 2u) & 0xFCu);
}

static void begin_reconfiguration(POWB_AVDTP_CONTEXT context,
                                  OWB_AVDTP_ACTION* action) {
    const unsigned char payload = seid_payload(context);
    context->State = OwbAvdtpStateSuspending;
    queue_command(context, action, OWB_AVDTP_MSG_SUSPEND, &payload, 1u);
}

void OwbAvdtpStateInit(POWB_AVDTP_CONTEXT context, unsigned char local_seid) {
    zero_bytes(context, sizeof(*context));
    context->State = OwbAvdtpStateIdle;
    context->LocalSeid = (unsigned char)(local_seid & 0x3Fu);
    if (context->LocalSeid == 0u) context->LocalSeid = 1u;
    context->ActiveCodecId = OWB_CODEC_SBC;
    context->PendingCodecId = OWB_CODEC_SBC;
    context->PreferredCodecId = OWB_CODEC_SBC;
    context->PreferredSampleRateHz = 44100u;
    context->PreferredChannels = 2u;
}

OWB_AVDTP_RESULT OwbAvdtpStateBeginDiscover(POWB_AVDTP_CONTEXT context,
                                            OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    if (context->ResponsePending ||
        (context->State != OwbAvdtpStateIdle &&
         context->State != OwbAvdtpStateConnecting)) {
        action_init(action);
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }
    context->State = OwbAvdtpStateDiscovering;
    queue_command(context, action, OWB_AVDTP_MSG_DISCOVER, NULL, 0u);
    return action->Result;
}

OWB_AVDTP_RESULT OwbAvdtpStateMediaOpened(POWB_AVDTP_CONTEXT context,
                                          int media_open_succeeded,
                                          OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    if (!media_open_succeeded || context->State != OwbAvdtpStateOpen ||
        context->ResponsePending) {
        return enter_failure(context, action, OwbAvdtpResultAborted);
    }
    const unsigned char payload = seid_payload(context);
    queue_command(context, action, OWB_AVDTP_MSG_START, &payload, 1u);
    return action->Result;
}

OWB_AVDTP_RESULT OwbAvdtpStateCommandFailed(POWB_AVDTP_CONTEXT context,
                                            OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    return enter_failure(context, action, OwbAvdtpResultAborted);
}

OWB_AVDTP_RESULT OwbAvdtpStateTimeout(POWB_AVDTP_CONTEXT context,
                                      OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    if (!context->ResponsePending) {
        action_init(action);
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }
    return enter_failure(context, action, OwbAvdtpResultAborted);
}

OWB_AVDTP_RESULT OwbAvdtpStateTick(POWB_AVDTP_CONTEXT context,
                                   OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    action_init(action);
    if (context->BackoffTicks > 0u) context->BackoffTicks--;
    if (!context->ResponsePending) {
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }
    if (context->PendingTimeoutTicks == 0u) {
        return enter_failure(context, action, OwbAvdtpResultAborted);
    }
    context->PendingTimeoutTicks--;
    if (context->PendingTimeoutTicks == 0u)
        return enter_failure(context, action, OwbAvdtpResultAborted);
    action->Result = OwbAvdtpResultIgnored;
    return action->Result;
}

OWB_AVDTP_RESULT OwbAvdtpStateSetPreferredCodec(POWB_AVDTP_CONTEXT context,
                                                unsigned long codec_id,
                                                OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    context->PreferredCodecId = codec_id;
    action_init(action);
    if (context->State != OwbAvdtpStateStreaming || context->ResponsePending) {
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }
    begin_reconfiguration(context, action);
    return action->Result;
}

OWB_AVDTP_RESULT OwbAvdtpStateSetPreferredFormat(POWB_AVDTP_CONTEXT context,
                                                 unsigned long sample_rate_hz,
                                                 unsigned long channels,
                                                 OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    context->PreferredSampleRateHz = sample_rate_hz;
    context->PreferredChannels = channels;
    action_init(action);
    if (context->State != OwbAvdtpStateStreaming || context->ResponsePending) {
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }
    begin_reconfiguration(context, action);
    return action->Result;
}

void OwbAvdtpStatePeerDisconnected(POWB_AVDTP_CONTEXT context,
                                   OWB_AVDTP_ACTION* action) {
    action_init(action);
    clear_pending(context);
    clear_fragment(context);
    context->State = OwbAvdtpStateIdle;
    context->ActiveCodecId = OWB_CODEC_SBC;
    context->PendingCodecId = OWB_CODEC_SBC;
    action->Result = OwbAvdtpResultAborted;
    action->Reconnect = 1;
}

int OwbAvdtpStateCanReconnect(const OWB_AVDTP_CONTEXT* context) {
    return context && context->State == OwbAvdtpStateIdle &&
           !context->ResponsePending && !context->FragmentActive &&
           context->BackoffTicks == 0u;
}

int OwbAvdtpStateCodecAllowed(const OWB_AVDTP_CONTEXT* context,
                              unsigned long codec_id) {
    return context && context->State == OwbAvdtpStateStreaming &&
           context->ActiveCodecId == codec_id;
}

const char* OwbAvdtpStateName(OWB_AVDTP_STATE state) {
    switch (state) {
        case OwbAvdtpStateIdle:        return "Idle";
        case OwbAvdtpStateConnecting:  return "Connecting";
        case OwbAvdtpStateDiscovering: return "Discovering";
        case OwbAvdtpStateConfiguring: return "Configuring";
        case OwbAvdtpStateConfigured:  return "Configured";
        case OwbAvdtpStateOpen:        return "Open";
        case OwbAvdtpStateStreaming:   return "Streaming";
        case OwbAvdtpStateSuspending:  return "Suspending";
        case OwbAvdtpStateClosing:     return "Closing";
        default:                       return "Unknown";
    }
}

static int fragment_matches(const OWB_AVDTP_CONTEXT* context, unsigned char txid,
                            unsigned char signal_id, unsigned char message_type) {
    return context->FragmentActive &&
           context->FragmentTransactionId == txid &&
           context->FragmentSignalId == signal_id &&
           context->FragmentMessageType == message_type;
}

static OWB_AVDTP_RESULT append_fragment(POWB_AVDTP_CONTEXT context,
                                        const unsigned char* payload,
                                        size_t payload_length) {
    if (payload_length > sizeof(context->FragmentBuffer) - context->FragmentLength)
        return OwbAvdtpResultProtocolError;
    if (payload_length > 0u)
        copy_bytes(context->FragmentBuffer + context->FragmentLength, payload,
                   payload_length);
    context->FragmentLength =
        (unsigned short)(context->FragmentLength + payload_length);
    return OwbAvdtpResultNone;
}

static OWB_AVDTP_RESULT handle_complete_message(
    POWB_AVDTP_CONTEXT context, unsigned char message_type, unsigned char txid,
    unsigned char signal_id, const unsigned char* payload, unsigned short payload_length,
    OWB_AVDTP_ACTION* action) {
    if (message_type == OWB_AVDTP_MSG_CMD) {
        if (signal_id == OWB_AVDTP_MSG_ABORT) {
            queue_response(action, OWB_AVDTP_MSG_ACCEPT, signal_id, txid);
            clear_pending(context);
            clear_fragment(context);
            context->State = OwbAvdtpStateIdle;
            context->ActiveCodecId = OWB_CODEC_SBC;
            context->PendingCodecId = OWB_CODEC_SBC;
            action->CloseMedia = 1;
            action->CloseChannels = 1;
            action->Reconnect = 1;
            action->Result = OwbAvdtpResultCommand;
            return action->Result;
        }
        // This source driver does not accept stream commands from the sink.
        queue_response(action, OWB_AVDTP_MSG_REJECT, signal_id, txid);
        return action->Result;
    }

    if (message_type != OWB_AVDTP_MSG_ACCEPT &&
        message_type != OWB_AVDTP_MSG_REJECT) {
        return enter_failure(context, action, OwbAvdtpResultProtocolError);
    }

    if (!context->ResponsePending ||
        txid != context->PendingTransactionId ||
        signal_id != context->PendingSignalId) {
        action_init(action);
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }

    if (message_type == OWB_AVDTP_MSG_REJECT) {
        return enter_failure(context, action, OwbAvdtpResultRejected);
    }

    clear_pending(context);
    action_init(action);

    switch (signal_id) {
        case OWB_AVDTP_MSG_DISCOVER: {
            const unsigned char seid =
                owb_avdtp_find_free_audio_sink_seid(payload, payload_length);
            if (seid == OWB_AVDTP_SEID_NONE)
                return enter_failure(context, action, OwbAvdtpResultAborted);
            context->RemoteSeid = seid;
            context->State = OwbAvdtpStateConfiguring;
            const unsigned char request =
                (unsigned char)(((unsigned int)seid << 2u) & 0xFCu);
            queue_command(context, action, OWB_AVDTP_MSG_GET_CAPABILITIES,
                          &request, 1u);
            return action->Result;
        }
        case OWB_AVDTP_MSG_GET_CAPABILITIES: {
            const unsigned long codec = owb_avdtp_select_codec(
                context->PreferredCodecId, payload, payload_length);
            unsigned char configuration[OWB_AVDTP_SET_CONFIGURATION_MAX_LEN];
            const size_t configuration_length = owb_avdtp_build_set_configuration_ex(
                codec, context->PreferredSampleRateHz, context->PreferredChannels,
                context->RemoteSeid, context->LocalSeid, configuration,
                sizeof(configuration));
            if (configuration_length == 0u)
                return enter_failure(context, action, OwbAvdtpResultProtocolError);
            context->PendingCodecId = codec;
            context->State = OwbAvdtpStateConfigured;
            queue_command(context, action, OWB_AVDTP_MSG_SET_CONFIGURATION,
                          configuration, (unsigned short)configuration_length);
            return action->Result;
        }
        case OWB_AVDTP_MSG_SET_CONFIGURATION: {
            const unsigned char request = seid_payload(context);
            context->ActiveCodecId = context->PendingCodecId;
            context->State = OwbAvdtpStateOpen;
            queue_command(context, action, OWB_AVDTP_MSG_OPEN, &request, 1u);
            return action->Result;
        }
        case OWB_AVDTP_MSG_OPEN:
            context->State = OwbAvdtpStateOpen;
            action->Result = OwbAvdtpResultOpenMedia;
            action->OpenMedia = 1;
            return action->Result;
        case OWB_AVDTP_MSG_START:
            context->State = OwbAvdtpStateStreaming;
            clear_failure(context);
            action->Result = OwbAvdtpResultNone;
            return action->Result;
        case OWB_AVDTP_MSG_SUSPEND: {
            const unsigned char request = seid_payload(context);
            context->State = OwbAvdtpStateClosing;
            queue_command(context, action, OWB_AVDTP_MSG_CLOSE, &request, 1u);
            return action->Result;
        }
        case OWB_AVDTP_MSG_CLOSE: {
            context->State = OwbAvdtpStateDiscovering;
            queue_command(context, action, OWB_AVDTP_MSG_DISCOVER, NULL, 0u);
            action->CloseMedia = 1;
            return action->Result;
        }
        case OWB_AVDTP_MSG_ABORT:
            context->State = OwbAvdtpStateIdle;
            action->CloseChannels = 1;
            action->Reconnect = 1;
            action->Result = OwbAvdtpResultAborted;
            return action->Result;
        default:
            return enter_failure(context, action, OwbAvdtpResultProtocolError);
    }
}

OWB_AVDTP_RESULT OwbAvdtpStateHandlePacket(POWB_AVDTP_CONTEXT context,
                                           const unsigned char* data,
                                           size_t length,
                                           OWB_AVDTP_ACTION* action) {
    if (!context || !action) return OwbAvdtpResultProtocolError;
    action_init(action);
    if (!data || length < 2u) {
        return enter_failure(context, action, OwbAvdtpResultProtocolError);
    }

    const unsigned char message_type = data[0] & 0x03u;
    const unsigned char transaction_id = (unsigned char)((data[0] >> 4u) & 0x0Fu);
    const unsigned char packet_type = (unsigned char)((data[0] >> 2u) & 0x03u);
    const unsigned char signal_id = data[1] & 0x3Fu;
    const unsigned char* payload = length > 2u ? data + 2u : NULL;
    const size_t payload_length = length > 2u ? length - 2u : 0u;

    if (packet_type == OWB_AVDTP_PKT_SINGLE) {
        if (context->FragmentActive)
            return enter_failure(context, action, OwbAvdtpResultProtocolError);
        return handle_complete_message(context, message_type, transaction_id,
                                       signal_id, payload,
                                       (unsigned short)payload_length, action);
    }

    if (packet_type == OWB_AVDTP_PKT_START) {
        if (context->FragmentActive)
            return enter_failure(context, action, OwbAvdtpResultProtocolError);
        context->FragmentActive = 1;
        context->FragmentTransactionId = transaction_id;
        context->FragmentSignalId = signal_id;
        context->FragmentMessageType = message_type;
        context->FragmentLength = 0u;
        if (append_fragment(context, payload, payload_length) != OwbAvdtpResultNone)
            return enter_failure(context, action, OwbAvdtpResultProtocolError);
        action->Result = OwbAvdtpResultIgnored;
        return action->Result;
    }

    if (packet_type == OWB_AVDTP_PKT_CONTINUE ||
        packet_type == OWB_AVDTP_PKT_END) {
        if (!fragment_matches(context, transaction_id, signal_id, message_type))
            return enter_failure(context, action, OwbAvdtpResultProtocolError);
        if (append_fragment(context, payload, payload_length) != OwbAvdtpResultNone)
            return enter_failure(context, action, OwbAvdtpResultProtocolError);
        if (packet_type == OWB_AVDTP_PKT_CONTINUE) {
            action->Result = OwbAvdtpResultIgnored;
            return action->Result;
        }

        unsigned char complete[OWB_AVDTP_SIGNAL_MAX];
        const unsigned short complete_length = context->FragmentLength;
        copy_bytes(complete, context->FragmentBuffer, complete_length);
        clear_fragment(context);
        return handle_complete_message(context, message_type, transaction_id,
                                       signal_id, complete, complete_length,
                                       action);
    }

    return enter_failure(context, action, OwbAvdtpResultProtocolError);
}
