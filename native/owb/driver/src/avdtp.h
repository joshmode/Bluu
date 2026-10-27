// driver/src/avdtp.h
// Thin WDF adapter around the host-tested AVDTP state machine.
#pragma once
#include <ntddk.h>
#include <wdf.h>

#include "avdtp_state.h"

// AVDTP L2CAP PSM. Media transport uses a distinct channel with the same PSM.
#define AVDTP_SIGNALING_PSM 0x0019u
#define AVDTP_MEDIA_PSM     AVDTP_SIGNALING_PSM

// A failed setup is retried by the reconnect timer after this many ticks.
#define OWB_AVDTP_REPLY_TIMEOUT_TICKS 2u

typedef struct _OWB_DEVICE_EXTENSION OWB_DEVICE_EXTENSION, *POWB_DEVICE_EXTENSION;

VOID AvdtpContextInit(POWB_AVDTP_CONTEXT Context);

NTSTATUS AvdtpConnect(POWB_DEVICE_EXTENSION DevExt);
NTSTATUS AvdtpSetPreferredCodec(POWB_DEVICE_EXTENSION DevExt, ULONG NewCodecId);
NTSTATUS AvdtpSetPreferredFormat(POWB_DEVICE_EXTENSION DevExt,
                                 ULONG SampleRateHz, ULONG Channels);

VOID AvdtpHandleSignalingPacket(POWB_DEVICE_EXTENSION DevExt,
                                const UCHAR* Data, USHORT Length);
VOID AvdtpHandleTick(POWB_DEVICE_EXTENSION DevExt);
VOID AvdtpHandlePeerDisconnected(POWB_DEVICE_EXTENSION DevExt);
NTSTATUS AvdtpHandleMediaOpened(POWB_DEVICE_EXTENSION DevExt, NTSTATUS MediaStatus);
