// driver/src/ioctl.c
// IOCTL dispatch — receives requests from owb-service.exe.
#include "ioctl.h"
#include "avdtp.h"
#include "l2cap_stream.h"
#include "owb_a2dp.h"
#include "owb_trace.h"
#include "../owb_ioctl.h"

NTSTATUS IoctlRegister(_In_ WDFDEVICE Device) {
    WDF_IO_QUEUE_CONFIG cfg;
    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&cfg, WdfIoQueueDispatchSequential);
    cfg.EvtIoDeviceControl = OwbEvtIoDeviceControl;

    WDFQUEUE queue;
    NTSTATUS status = WdfIoQueueCreate(Device, &cfg, WDF_NO_OBJECT_ATTRIBUTES, &queue);
    OwbLog("IoctlRegister: cola IOCTL -> 0x%x", status);
    return status;
}

VOID OwbEvtIoDeviceControl(
    _In_ WDFQUEUE   Queue,
    _In_ WDFREQUEST Request,
    _In_ size_t     OutputBufferLength,
    _In_ size_t     InputBufferLength,
    _In_ ULONG      IoControlCode)
{
    WDFDEVICE            device  = WdfIoQueueGetDevice(Queue);
    POWB_DEVICE_EXTENSION devExt = OwbGetDeviceExtension(device);
    NTSTATUS             status  = STATUS_SUCCESS;
    ULONG_PTR            info    = 0;

    switch (IoControlCode) {

        case OWB_IOCTL_SEND_AUDIO_FRAME: {
            PVOID   buf  = NULL;
            size_t  size = 0;
            status = WdfRequestRetrieveInputBuffer(
                Request, sizeof(OWB_SEND_FRAME_INPUT), &buf, &size);
            if (!NT_SUCCESS(status)) {
                OwbLog("SEND_FRAME: RetrieveInputBuffer fallo 0x%x (in=%Iu)",
                       status, InputBufferLength);
                break;
            }

            POWB_SEND_FRAME_INPUT frame = (POWB_SEND_FRAME_INPUT)buf;
            // Validate: data_len must fit in the input buffer AND not overflow
            // the RTP packet length (OWB_RTP_OVERHEAD + data_len <= 0xFFFF).
            if ((SIZE_T)frame->data_len > size - offsetof(OWB_SEND_FRAME_INPUT, data) ||
                frame->data_len > 0xFFFFu - OWB_RTP_OVERHEAD) {
                OwbLog("SEND_FRAME: data_len invalido %lu (buf=%Iu)",
                       frame->data_len, size);
                status = STATUS_INVALID_PARAMETER;
                break;
            }
            status = L2capSendMediaFrame(devExt,
                                         frame->codec_id,
                                         frame->data,
                                         (USHORT)frame->data_len);

            devExt->FrameCount++;
            if (status != devExt->LastFrameStatus) {
                OwbLog("SEND_FRAME: status cambio 0x%x -> 0x%x (frame #%lu codec=%lu len=%lu)",
                       devExt->LastFrameStatus, status, devExt->FrameCount,
                       frame->codec_id, frame->data_len);
                devExt->LastFrameStatus = status;
            } else if ((devExt->FrameCount % 1000u) == 0u) {
                OwbLog("SEND_FRAME: %lu frames (codec=%lu len=%lu status=0x%x)",
                       devExt->FrameCount, frame->codec_id, frame->data_len, status);
            }
            break;
        }

        case OWB_IOCTL_GET_RF_QUALITY: {
            PVOID   buf  = NULL;
            size_t  size = 0;
            status = WdfRequestRetrieveOutputBuffer(
                Request, sizeof(OWB_RF_QUALITY), &buf, &size);
            if (!NT_SUCCESS(status)) {
                OwbLog("GET_RF_QUALITY: RetrieveOutputBuffer fallo 0x%x", status);
                break;
            }

            status = STATUS_NOT_SUPPORTED;
            break;
        }

        case OWB_IOCTL_SET_CODEC_CONFIG: {
            PVOID   buf  = NULL;
            size_t  size = 0;
            status = WdfRequestRetrieveInputBuffer(
                Request, sizeof(OWB_CODEC_CONFIG), &buf, &size);
            if (!NT_SUCCESS(status)) {
                OwbLog("SET_CODEC_CONFIG: RetrieveInputBuffer fallo 0x%x", status);
                break;
            }

            POWB_CODEC_CONFIG cfg = (POWB_CODEC_CONFIG)buf;
            OwbLog("SET_CODEC_CONFIG: codec=%lu key=%.16s val=%lld",
                   cfg->codec_id, cfg->param_key, cfg->param_value);

            // Only the exact "switch" key changes codec negotiation. Parameter
            // validation remains in the user-mode encoder/driver negotiation path.
            if (RtlEqualMemory(cfg->param_key, "switch", 6)) {
                status = AvdtpSetPreferredCodec(devExt, cfg->codec_id);
                OwbLog("SET_CODEC_CONFIG: AvdtpSetPreferredCodec -> 0x%x", status);
            } else if (RtlEqualMemory(cfg->param_key, "quality", 7) ||
                       RtlEqualMemory(cfg->param_key, "freq", 4) ||
                       RtlEqualMemory(cfg->param_key, "bitpool", 7)) {
                // Encoder parameters are owned by the service. Only "switch"
                // triggers AVDTP renegotiation, so acknowledge these without
                // pretending the driver rewrote a codec configuration.
                status = STATUS_SUCCESS;
            } else {
                status = STATUS_INVALID_PARAMETER;
            }
            break;
        }

        case OWB_IOCTL_GET_DEVICE_STATE: {
            PVOID   buf  = NULL;
            size_t  size = 0;
            status = WdfRequestRetrieveOutputBuffer(
                Request, sizeof(OWB_DEVICE_STATE), &buf, &size);
            if (!NT_SUCCESS(status)) {
                OwbLog("GET_DEVICE_STATE: RetrieveOutputBuffer fallo 0x%x", status);
                break;
            }

            OWB_DEVICE_STATE* state = (OWB_DEVICE_STATE*)buf;
            state->state           = (ULONG)devExt->Avdtp.State;
            state->active_codec_id = devExt->Avdtp.ActiveCodecId;
            RtlZeroMemory(state->remote_addr, sizeof(state->remote_addr));
            info = sizeof(OWB_DEVICE_STATE);

            devExt->StateQueryCount++;
            if ((devExt->StateQueryCount % 50u) == 1u) {
                OwbLog("GET_DEVICE_STATE: #%lu estado=%s codec=%lu",
                       devExt->StateQueryCount, OwbAvdtpStateName(devExt->Avdtp.State),
                       devExt->Avdtp.ActiveCodecId);
            }
            break;
        }

        default:
            OwbLog("IOCTL desconocido 0x%08lx (in=%Iu out=%Iu)",
                   IoControlCode, InputBufferLength, OutputBufferLength);
            status = STATUS_INVALID_DEVICE_REQUEST;
            break;
    }

    WdfRequestCompleteWithInformation(Request, status, info);
}
