// driver/src/l2cap_stream.c
// L2CAP channel management — Phase 2c real BRB implementation.
// Reference: Windows Driver Samples / bluetooth / bthecho (MIT License)
#include "l2cap_stream.h"
#include "avdtp.h"
#include "owb_a2dp.h"
#include "owb_trace.h"
#include "../owb_ioctl.h"   // OWB_CODEC_* ids
#include <bthioctl.h>   // IOCTL_INTERNAL_BTH_SUBMIT_BRB

// RTP header (12 bytes packed, RFC 3550).
#pragma pack(push, 1)
typedef struct _OWB_RTP_HEADER {
    UCHAR   vpxcc;      // V=2 P=0 X=0 CC=0 -> 0x80
    UCHAR   mpt;        // M=0 PT=96 (SBC) -> 0x60
    USHORT  seq_num;    // big-endian sequence number
    ULONG   timestamp;  // big-endian sample clock
    ULONG   ssrc;       // big-endian synchronization source
} OWB_RTP_HEADER;
#pragma pack(pop)

// BRB submit helper
// Submits a BRB to BthPort synchronously.
// MUST be called at PASSIVE_LEVEL only (connection setup, not streaming hot path).
NTSTATUS L2capSubmitBrb(
    _In_ POWB_DEVICE_EXTENSION DevExt,
    _In_ PBRB                  Brb,
    _In_ ULONG                 BrbSize)
{
    // Serializar: BrbRequest es único y compartido; dos WdfRequestReuse
    // concurrentes sobre un request en vuelo corrompen y crashean.
    WdfWaitLockAcquire(DevExt->BrbLock, NULL);

    WDF_REQUEST_REUSE_PARAMS reuseParams;
    WDF_REQUEST_REUSE_PARAMS_INIT(&reuseParams,
                                   WDF_REQUEST_REUSE_NO_FLAGS,
                                   STATUS_NOT_SUPPORTED);
    NTSTATUS status = WdfRequestReuse(DevExt->BrbRequest, &reuseParams);
    if (!NT_SUCCESS(status)) {
        OwbLog("SubmitBrb: WdfRequestReuse fallo 0x%x", status);
        WdfWaitLockRelease(DevExt->BrbLock);
        return status;
    }

    WDF_MEMORY_DESCRIPTOR memDesc;
    WDF_MEMORY_DESCRIPTOR_INIT_BUFFER(&memDesc, Brb, BrbSize);

    status = WdfIoTargetSendInternalIoctlOthersSynchronously(
        DevExt->BthIoTarget,
        DevExt->BrbRequest,
        IOCTL_INTERNAL_BTH_SUBMIT_BRB,
        &memDesc,
        NULL,   // OtherArg2
        NULL,   // OtherArg4
        NULL,   // RequestOptions
        NULL    // BytesReturned
    );

    WdfWaitLockRelease(DevExt->BrbLock);
    return status;
}

static L2CAP_CHANNEL_HANDLE signaling_handle(_In_ POWB_DEVICE_EXTENSION DevExt) {
    return (L2CAP_CHANNEL_HANDLE)InterlockedCompareExchangePointer(
        (PVOID volatile*)&DevExt->SignalingChannelHandle, NULL, NULL);
}

static L2CAP_CHANNEL_HANDLE media_handle(_In_ POWB_DEVICE_EXTENSION DevExt) {
    return (L2CAP_CHANNEL_HANDLE)InterlockedCompareExchangePointer(
        (PVOID volatile*)&DevExt->MediaChannelHandle, NULL, NULL);
}

L2CAP_CHANNEL_HANDLE L2capSignalingChannelHandle(_In_ POWB_DEVICE_EXTENSION DevExt) {
    return signaling_handle(DevExt);
}

L2CAP_CHANNEL_HANDLE L2capMediaChannelHandle(_In_ POWB_DEVICE_EXTENSION DevExt) {
    return media_handle(DevExt);
}

// Signaling receive callback. Called at up to DISPATCH_LEVEL; only spinlock-safe
// queueing is done here. All BRB work happens in the PASSIVE work item.
_IRQL_requires_max_(DISPATCH_LEVEL)
VOID L2capSignalingIndicationCallback(
    _In_opt_ PVOID                  Context,
    _In_     INDICATION_CODE        Indication,
    _In_     PINDICATION_PARAMETERS Parameters)
{
    POWB_DEVICE_EXTENSION devExt = (POWB_DEVICE_EXTENSION)Context;
    if (!devExt) return;

    switch (Indication) {
        case IndicationRecvPacket:
            if (signaling_handle(devExt) == NULL || !devExt->AvdtpWorkItem ||
                InterlockedCompareExchange(&devExt->ClosingChannels, 0, 0) != 0) {
                OwbLog("L2CAP ind: RecvPacket ignored (channel/device closing)");
                break;
            }
            const ULONG length = Parameters->Parameters.RecvPacket.PacketLength;
            if (length == 0) break;
            WdfSpinLockAcquire(devExt->RecvLock);
            if (!OwbRecvQueuePush(&devExt->RecvQueue, length)) {
                InterlockedExchange(&devExt->ProtocolFault, 1);
                OwbLog("L2CAP ind: receive queue overflow");
            }
            const BOOLEAN queued = OwbRecvQueueCount(&devExt->RecvQueue) != 0u;
            WdfSpinLockRelease(devExt->RecvLock);
            if (queued) WdfWorkItemEnqueue(devExt->AvdtpWorkItem);
            break;

        case IndicationRemoteDisconnect:
            // Do not submit BRBs at DISPATCH_LEVEL. Forget the handles now and
            // let the PASSIVE reconnect worker renegotiate from scratch.
            L2capForgetChannels(devExt);
            devExt->RtpSeqNum              = 0;
            devExt->RtpTimestamp           = 0;
            AvdtpHandlePeerDisconnected(devExt);
            WdfWorkItemEnqueue(devExt->ReconnectWorkItem);
            OwbLog("L2CAP ind: canal de signaling DESCONECTADO por remoto");
            break;

        default:
            OwbLog("L2CAP ind: no manejada (%d)", (int)Indication);
            break;
    }
}

// L2CAP signaling channel open
NTSTATUS L2capOpenSignalingChannel(_In_ POWB_DEVICE_EXTENSION DevExt)
{
    if (!DevExt->BthInterface.BthAllocateBrb) {
        OwbLog("OpenSignaling: sin interfaz BthPort (DEVICE_NOT_READY)");
        return STATUS_DEVICE_NOT_READY;
    }
    if (DevExt->RemoteBtAddress == 0) {
        OwbLog("OpenSignaling: sin BT addr remota (DEVICE_NOT_CONNECTED)");
        return STATUS_DEVICE_NOT_CONNECTED;
    }
    OwbLog("OpenSignaling: abriendo PSM 0x%04x hacia %012I64x",
           AVDTP_SIGNALING_PSM, DevExt->RemoteBtAddress);

    struct _BRB_L2CA_OPEN_CHANNEL* brb =
        (struct _BRB_L2CA_OPEN_CHANNEL*)
        DevExt->BthInterface.BthAllocateBrb(BRB_L2CA_OPEN_CHANNEL, 'OWBO');
    if (!brb) return STATUS_INSUFFICIENT_RESOURCES;

    brb->BtAddress            = DevExt->RemoteBtAddress;
    brb->Psm                  = AVDTP_SIGNALING_PSM;
    brb->ChannelFlags         = CF_LINK_ENCRYPTED | CF_LINK_AUTHENTICATED;
    brb->ConfigIn.Mtu.Max     = L2CAP_DEFAULT_MTU;
    brb->ConfigIn.Mtu.Min     = L2CAP_MIN_MTU;
    brb->ConfigOut.Mtu.Max    = L2CAP_DEFAULT_MTU;
    brb->ConfigOut.Mtu.Min    = L2CAP_MIN_MTU;
    brb->Callback             = L2capSignalingIndicationCallback;
    brb->CallbackContext      = DevExt;
    brb->ReferenceObject      = (PVOID)WdfDeviceWdmGetDeviceObject(DevExt->Device);
    brb->CallbackFlags        = CALLBACK_DISCONNECT | CALLBACK_RECV_PACKET;

    NTSTATUS status = L2capSubmitBrb(DevExt, (PBRB)brb, sizeof(*brb));
    if (NT_SUCCESS(status)) {
        InterlockedExchangePointer((PVOID volatile*)&DevExt->SignalingChannelHandle,
                                   brb->ChannelHandle);
        DevExt->Avdtp.State = OwbAvdtpStateConnecting;
        OwbLog("OpenSignaling: canal abierto handle=0x%p", (PVOID)brb->ChannelHandle);
        status = AvdtpConnect(DevExt);
    } else {
        OwbLog("OpenSignaling: BRB_L2CA_OPEN_CHANNEL fallo 0x%x", status);
    }

    DevExt->BthInterface.BthFreeBrb((PBRB)brb);
    return status;
}

// L2CAP media channel open (after AVDTP OPEN)
NTSTATUS L2capOpenMediaChannel(_In_ POWB_DEVICE_EXTENSION DevExt)
{
    if (!DevExt->BthInterface.BthAllocateBrb) {
        OwbLog("OpenMedia: sin interfaz BthPort");
        return STATUS_DEVICE_NOT_READY;
    }
    if (DevExt->RemoteBtAddress == 0) {
        OwbLog("OpenMedia: sin BT addr remota");
        return STATUS_DEVICE_NOT_CONNECTED;
    }
    OwbLog("OpenMedia: abriendo PSM 0x%04x", AVDTP_MEDIA_PSM);

    struct _BRB_L2CA_OPEN_CHANNEL* brb =
        (struct _BRB_L2CA_OPEN_CHANNEL*)
        DevExt->BthInterface.BthAllocateBrb(BRB_L2CA_OPEN_CHANNEL, 'OWBM');
    if (!brb) return STATUS_INSUFFICIENT_RESOURCES;

    brb->BtAddress            = DevExt->RemoteBtAddress;
    brb->Psm                  = AVDTP_MEDIA_PSM;
    brb->ChannelFlags         = CF_LINK_ENCRYPTED | CF_LINK_AUTHENTICATED;
    brb->ConfigIn.Mtu.Max     = L2CAP_DEFAULT_MTU;
    brb->ConfigIn.Mtu.Min     = L2CAP_MIN_MTU;
    brb->ConfigOut.Mtu.Max    = L2CAP_DEFAULT_MTU;
    brb->ConfigOut.Mtu.Min    = L2CAP_MIN_MTU;
    brb->Callback             = NULL;   // A2DP source: send-only
    brb->CallbackContext      = NULL;
    brb->ReferenceObject      = NULL;
    brb->CallbackFlags        = 0;

    NTSTATUS status = L2capSubmitBrb(DevExt, (PBRB)brb, sizeof(*brb));
    if (NT_SUCCESS(status)) {
        InterlockedExchangePointer((PVOID volatile*)&DevExt->MediaChannelHandle,
                                   brb->ChannelHandle);
        OwbLog("OpenMedia: canal abierto handle=0x%p", (PVOID)brb->ChannelHandle);
    } else {
        OwbLog("OpenMedia: BRB_L2CA_OPEN_CHANNEL fallo 0x%x", status);
    }

    DevExt->BthInterface.BthFreeBrb((PBRB)brb);
    return status;
}

static void close_l2cap_channel(_In_ POWB_DEVICE_EXTENSION DevExt,
                                _Inout_ L2CAP_CHANNEL_HANDLE* Handle,
                                ULONG Tag) {
    L2CAP_CHANNEL_HANDLE channel =
        (L2CAP_CHANNEL_HANDLE)InterlockedExchangePointer((PVOID volatile*)Handle, NULL);
    if (channel == NULL) return;

    struct _BRB_L2CA_CLOSE_CHANNEL* brb =
        (struct _BRB_L2CA_CLOSE_CHANNEL*)
        DevExt->BthInterface.BthAllocateBrb(BRB_L2CA_CLOSE_CHANNEL, Tag);
    if (brb) {
        brb->BtAddress = DevExt->RemoteBtAddress;
        brb->ChannelHandle = channel;
        const NTSTATUS status =
            L2capSubmitBrb(DevExt, (PBRB)brb, sizeof(*brb));
        if (!NT_SUCCESS(status))
            OwbLog("L2CAP close channel failed 0x%x", status);
        DevExt->BthInterface.BthFreeBrb((PBRB)brb);
    } else {
        OwbLog("L2CAP close channel allocation failed");
    }
}

VOID L2capPrepareCloseMediaChannel(_In_ POWB_DEVICE_EXTENSION DevExt) {
    if (!DevExt->BthInterface.BthAllocateBrb) return;
    close_l2cap_channel(DevExt, &DevExt->MediaChannelHandle, 'OWCM');
}

VOID L2capPrepareCloseChannels(_In_ POWB_DEVICE_EXTENSION DevExt) {
    if (!DevExt->BthInterface.BthAllocateBrb) return;
    InterlockedExchange(&DevExt->ClosingChannels, 1);
    WdfSpinLockAcquire(DevExt->RecvLock);
    OwbRecvQueueClear(&DevExt->RecvQueue);
    WdfSpinLockRelease(DevExt->RecvLock);
    close_l2cap_channel(DevExt, &DevExt->MediaChannelHandle, 'OWCM');
    close_l2cap_channel(DevExt, &DevExt->SignalingChannelHandle, 'OWCS');
    InterlockedExchange(&DevExt->ClosingChannels, 0);
}

VOID L2capForgetChannels(_In_ POWB_DEVICE_EXTENSION DevExt) {
    InterlockedExchangePointer((PVOID volatile*)&DevExt->MediaChannelHandle, NULL);
    InterlockedExchangePointer((PVOID volatile*)&DevExt->SignalingChannelHandle, NULL);
    InterlockedExchange(&DevExt->ClosingChannels, 1);
    WdfSpinLockAcquire(DevExt->RecvLock);
    OwbRecvQueueClear(&DevExt->RecvQueue);
    WdfSpinLockRelease(DevExt->RecvLock);
}

VOID L2capClearForReconnect(_In_ POWB_DEVICE_EXTENSION DevExt) {
    InterlockedExchange(&DevExt->ClosingChannels, 0);
}

// L2CAP signaling send
NTSTATUS L2capSendSignaling(
    _In_ POWB_DEVICE_EXTENSION DevExt,
    _In_reads_bytes_(Length) const UCHAR* Data,
    _In_ USHORT Length)
{
    if (!DevExt->BthInterface.BthAllocateBrb) {
        OwbLog("SendSignaling: sin interfaz BthPort");
        return STATUS_DEVICE_NOT_READY;
    }
    const L2CAP_CHANNEL_HANDLE channel = signaling_handle(DevExt);
    if (channel == NULL) {
        OwbLog("SendSignaling: canal de signaling cerrado");
        return STATUS_DEVICE_NOT_CONNECTED;
    }

    struct _BRB_L2CA_ACL_TRANSFER* brb =
        (struct _BRB_L2CA_ACL_TRANSFER*)
        DevExt->BthInterface.BthAllocateBrb(BRB_L2CA_ACL_TRANSFER, 'OWBS');
    if (!brb) return STATUS_INSUFFICIENT_RESOURCES;

    brb->BtAddress     = DevExt->RemoteBtAddress;
    brb->ChannelHandle = channel;
    brb->TransferFlags = ACL_TRANSFER_DIRECTION_OUT | ACL_SHORT_TRANSFER_OK;
    brb->BufferSize    = Length;
    brb->Buffer        = (PVOID)Data;
    brb->BufferMDL     = NULL;

    NTSTATUS status = L2capSubmitBrb(DevExt, (PBRB)brb, sizeof(*brb));
    if (!NT_SUCCESS(status)) {
        OwbLog("SendSignaling: fallo 0x%x len=%u", status, Length);
    }
    DevExt->BthInterface.BthFreeBrb((PBRB)brb);
    return status;
}

// L2CAP media frame send
NTSTATUS L2capSendMediaFrame(
    _In_ POWB_DEVICE_EXTENSION DevExt,
    _In_ ULONG  CodecId,
    _In_reads_bytes_(FrameLen) const UCHAR* FrameData,
    _In_ USHORT FrameLen)
{
    if (!OwbAvdtpStateCodecAllowed(&DevExt->Avdtp, CodecId)) {
        DevExt->MediaNotStreamingCount++;
        if ((DevExt->MediaNotStreamingCount % 1000u) == 1u) {
            OwbLog("SendMedia: descartado #%lu - estado=%s codec=%lu active=%lu",
                   DevExt->MediaNotStreamingCount,
                   OwbAvdtpStateName(DevExt->Avdtp.State),
                   CodecId, DevExt->Avdtp.ActiveCodecId);
        }
        return STATUS_DEVICE_NOT_CONNECTED;
    }
    if (!DevExt->BthInterface.BthAllocateBrb) {
        OwbLog("SendMedia: sin interfaz BthPort");
        return STATUS_DEVICE_NOT_READY;
    }
    const L2CAP_CHANNEL_HANDLE channel = media_handle(DevExt);
    if (channel == NULL) return STATUS_DEVICE_NOT_CONNECTED;

    int use_rtp = 1, payload_hdr = 1;
    ULONG   ts_incr = 128u;
    owb_avdtp_media_framing(CodecId, &use_rtp, &payload_hdr, &ts_incr);

    const ULONG rtp_len = use_rtp ? (ULONG)sizeof(OWB_RTP_HEADER) : 0u;
    const ULONG hdr_len = (ULONG)payload_hdr;

    ULONG pkt_len_u = rtp_len + hdr_len + (ULONG)FrameLen;
    if (pkt_len_u > 0xFFFFu) return STATUS_INVALID_PARAMETER;
    const USHORT pkt_len = (USHORT)pkt_len_u;

    PUCHAR pkt = (PUCHAR)ExAllocatePool2(POOL_FLAG_NON_PAGED,
                                          (SIZE_T)pkt_len, 'RTPM');
    if (!pkt) return STATUS_INSUFFICIENT_RESOURCES;

    if (use_rtp) {
        OWB_RTP_HEADER* hdr = (OWB_RTP_HEADER*)pkt;
        hdr->vpxcc     = 0x80u;
        hdr->mpt       = 0x60u;
        hdr->seq_num   = RtlUshortByteSwap(DevExt->RtpSeqNum++);
        hdr->timestamp = RtlUlongByteSwap(DevExt->RtpTimestamp);
        hdr->ssrc      = RtlUlongByteSwap(0x00000001UL);
        DevExt->RtpTimestamp += ts_incr;
    }
    if (payload_hdr == 1) pkt[rtp_len] = 0x01u;  // SBC/LDAC: 1 frame
    RtlCopyMemory(pkt + rtp_len + hdr_len, FrameData, FrameLen);

    struct _BRB_L2CA_ACL_TRANSFER* brb =
        (struct _BRB_L2CA_ACL_TRANSFER*)
        DevExt->BthInterface.BthAllocateBrb(BRB_L2CA_ACL_TRANSFER, 'BRTM');
    if (!brb) {
        ExFreePoolWithTag(pkt, 'RTPM');
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    brb->BtAddress     = DevExt->RemoteBtAddress;
    brb->ChannelHandle = channel;
    brb->TransferFlags = ACL_TRANSFER_DIRECTION_OUT | ACL_SHORT_TRANSFER_OK;
    brb->BufferSize    = pkt_len;
    brb->Buffer        = pkt;
    brb->BufferMDL     = NULL;

    NTSTATUS status = L2capSubmitBrb(DevExt, (PBRB)brb, sizeof(*brb));
    DevExt->BthInterface.BthFreeBrb((PBRB)brb);
    ExFreePoolWithTag(pkt, 'RTPM');

    DevExt->MediaPacketCount++;
    if (DevExt->MediaPacketCount == 1u) {
        OwbLog("SendMedia: PRIMER paquete (codec=%lu pkt=%u rtp=%d status=0x%x)",
               CodecId, pkt_len, use_rtp, status);
    } else if (!NT_SUCCESS(status) || (DevExt->MediaPacketCount % 1000u) == 0u) {
        OwbLog("SendMedia: #%lu codec=%lu pkt=%u status=0x%x",
               DevExt->MediaPacketCount, CodecId, pkt_len, status);
    }
    return status;
}

// RSSI query
NTSTATUS L2capGetRssi(
    _In_  POWB_DEVICE_EXTENSION DevExt,
    _Out_ LONG*                 RssiDbm)
{
    if (RssiDbm) *RssiDbm = 0;
    UNREFERENCED_PARAMETER(DevExt);
    return STATUS_NOT_SUPPORTED;
}
