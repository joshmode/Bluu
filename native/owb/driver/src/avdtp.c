// driver/src/avdtp.c
// WDF adapter for AVDTP commands and completed actions.
#include "avdtp.h"
#include "l2cap_stream.h"
#include "owb_a2dp.h"
#include "owb_trace.h"

static NTSTATUS send_action(_In_ POWB_DEVICE_EXTENSION DevExt,
                            _In_ const OWB_AVDTP_ACTION* Action) {
    if (!Action->HasCommand) return STATUS_SUCCESS;

    UCHAR packet[2u + OWB_AVDTP_SET_CONFIGURATION_MAX_LEN];
    const USHORT length = (USHORT)(2u + Action->PayloadLen);
    if (Action->PayloadLen > OWB_AVDTP_SET_CONFIGURATION_MAX_LEN)
        return STATUS_INVALID_PARAMETER;
    packet[0] = (UCHAR)((Action->TransactionId << 4u) |
                        (OWB_AVDTP_PKT_SINGLE << 2u) | Action->MessageType);
    packet[1] = (UCHAR)(Action->SignalId & 0x3Fu);
    if (Action->PayloadLen > 0u)
        RtlCopyMemory(packet + 2u, Action->Payload, Action->PayloadLen);

    NTSTATUS status = L2capSendSignaling(DevExt, packet, length);
    if (!NT_SUCCESS(status)) {
        OwbLog("AVDTP TX: signal=0x%02x txid=%u len=%u failed 0x%x",
               Action->SignalId, Action->TransactionId, length, status);
        return status;
    }
    OwbLog("AVDTP TX: signal=0x%02x txid=%u type=%u len=%u",
           Action->SignalId, Action->TransactionId, Action->MessageType, length);
    return STATUS_SUCCESS;
}

static NTSTATUS execute_action(_In_ POWB_DEVICE_EXTENSION DevExt,
                               _In_ const OWB_AVDTP_ACTION* Action) {
    NTSTATUS status = send_action(DevExt, Action);
    if (!NT_SUCCESS(status)) {
        OWB_AVDTP_ACTION failure = {0};
        OwbAvdtpStateCommandFailed(&DevExt->Avdtp, &failure);
        L2capPrepareCloseChannels(DevExt);
        return status;
    }
    if (Action->CloseMedia) L2capPrepareCloseMediaChannel(DevExt);
    if (Action->CloseChannels) L2capPrepareCloseChannels(DevExt);
    if (Action->OpenMedia) {
        status = L2capOpenMediaChannel(DevExt);
        return AvdtpHandleMediaOpened(DevExt, status);
    }
    return STATUS_SUCCESS;
}

static NTSTATUS run_action(_In_ POWB_DEVICE_EXTENSION DevExt,
                           _In_ OWB_AVDTP_RESULT Result,
                           _In_ const OWB_AVDTP_ACTION* Action) {
    if (Result == OwbAvdtpResultProtocolError ||
        Result == OwbAvdtpResultRejected || Result == OwbAvdtpResultAborted) {
        OwbLog("AVDTP: state=%s result=%d", OwbAvdtpStateName(DevExt->Avdtp.State),
               (int)Result);
    }
    return execute_action(DevExt, Action);
}

VOID AvdtpContextInit(POWB_AVDTP_CONTEXT Context) {
    OwbAvdtpStateInit(Context, 0x01u);
    OwbLog("AVDTP: context initialized (local SEID=0x%02x)", Context->LocalSeid);
}

NTSTATUS AvdtpConnect(POWB_DEVICE_EXTENSION DevExt) {
    OWB_AVDTP_ACTION action = {0};
    OwbAvdtpStateSetPreferredFormat(&DevExt->Avdtp, DevExt->SampleRateHz, 2u,
                                    &action);
    const OWB_AVDTP_RESULT result =
        OwbAvdtpStateBeginDiscover(&DevExt->Avdtp, &action);
    return run_action(DevExt, result, &action);
}

NTSTATUS AvdtpSetPreferredCodec(POWB_DEVICE_EXTENSION DevExt, ULONG NewCodecId) {
    OwbLog("AVDTP: preferred codec %lu (state=%s)", NewCodecId,
           OwbAvdtpStateName(DevExt->Avdtp.State));
    OWB_AVDTP_ACTION action = {0};
    const OWB_AVDTP_RESULT result =
        OwbAvdtpStateSetPreferredCodec(&DevExt->Avdtp, NewCodecId, &action);
    return run_action(DevExt, result, &action);
}

NTSTATUS AvdtpSetPreferredFormat(POWB_DEVICE_EXTENSION DevExt,
                                 ULONG SampleRateHz, ULONG Channels) {
    OWB_AVDTP_ACTION action = {0};
    const OWB_AVDTP_RESULT result = OwbAvdtpStateSetPreferredFormat(
        &DevExt->Avdtp, SampleRateHz, Channels, &action);
    return run_action(DevExt, result, &action);
}

VOID AvdtpHandleSignalingPacket(POWB_DEVICE_EXTENSION DevExt,
                                const UCHAR* Data, USHORT Length) {
    OWB_AVDTP_ACTION action = {0};
    const OWB_AVDTP_RESULT result =
        OwbAvdtpStateHandlePacket(&DevExt->Avdtp, Data, Length, &action);
    if (result != OwbAvdtpResultIgnored) {
        OwbLog("AVDTP RX: result=%d state=%s", (int)result,
               OwbAvdtpStateName(DevExt->Avdtp.State));
    }
    (VOID)run_action(DevExt, result, &action);
}

VOID AvdtpHandleTick(POWB_DEVICE_EXTENSION DevExt) {
    OWB_AVDTP_ACTION action = {0};
    const OWB_AVDTP_RESULT result = OwbAvdtpStateTick(&DevExt->Avdtp, &action);
    if (result != OwbAvdtpResultIgnored) (VOID)run_action(DevExt, result, &action);
}

VOID AvdtpHandlePeerDisconnected(POWB_DEVICE_EXTENSION DevExt) {
    OWB_AVDTP_ACTION action = {0};
    OwbAvdtpStatePeerDisconnected(&DevExt->Avdtp, &action);
}

NTSTATUS AvdtpHandleMediaOpened(POWB_DEVICE_EXTENSION DevExt, NTSTATUS MediaStatus) {
    OWB_AVDTP_ACTION action = {0};
    const OWB_AVDTP_RESULT result = OwbAvdtpStateMediaOpened(
        &DevExt->Avdtp, NT_SUCCESS(MediaStatus), &action);
    return run_action(DevExt, result, &action);
}
