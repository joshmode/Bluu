// driver/src/owb_a2dp.h
#pragma once
#include <ntddk.h>
#include <wdf.h>
// Correct Bluetooth WDK include order per official bthecho sample:
#include <initguid.h>   // instantiates GUIDs (GUID_BTHDDI_PROFILE_DRIVER_INTERFACE etc.)
#include <bthdef.h>     // BTH_ADDR, BTHSTATUS
#include <bthguid.h>    // BT service GUIDs
#include <bthioctl.h>   // BT IOCTL codes
#include <bthddi.h>     // BTH_PROFILE_DRIVER_INTERFACE, BRB types, L2CAP_CHANNEL_HANDLE
#include "avdtp.h"
#include "recv_queue.h"

#define OWB_DRIVER_VERSION_MAJOR 0
#define OWB_DRIVER_VERSION_MINOR 3

typedef struct _OWB_DEVICE_EXTENSION {
    WDFDEVICE                    Device;
    BOOLEAN                      IsActive;

    // Remote Bluetooth device address (read from PnP DevicePropertyAddress)
    BTH_ADDR                     RemoteBtAddress;

    // BthPort profile driver interface (BthAllocateBrb / BthFreeBrb)
    BTH_PROFILE_DRIVER_INTERFACE BthInterface;

    // IoTarget for submitting BRBs to the Bluetooth stack
    WDFIOTARGET                  BthIoTarget;

    // Pre-allocated WDFREQUEST reused for synchronous BRB submission.
    // Created once in EvtDeviceAdd, reused via WdfRequestReuse each time.
    WDFREQUEST                   BrbRequest;

    // Serializa el uso de BrbRequest: es un único request compartido por todos
    // los paths (open signaling/media, send signaling, send media/SEND_FRAME,
    // BRB IN). Dos WdfRequestReuse concurrentes sobre un request en vuelo
    // corrompen y crashean (0xC0000005) — pasaba en la reconexión mientras el
    // servicio seguía mandando frames. Todos los submits son a PASSIVE_LEVEL.
    WDFWAITLOCK                  BrbLock;

    // L2CAP channel handles. Accessed atomically because receive/disconnect
    // callbacks can run at DISPATCH_LEVEL while teardown
    // runs at PASSIVE_LEVEL.
    L2CAP_CHANNEL_HANDLE         SignalingChannelHandle;
    L2CAP_CHANNEL_HANDLE         MediaChannelHandle;

    // AVDTP signaling state machine context
    OWB_AVDTP_CONTEXT            Avdtp;

    ULONG                        SampleRateHz;
    ULONG                        MediaPacketCount;
    ULONG                        MediaNotStreamingCount;
    ULONG                        FrameCount;
    ULONG                        StateQueryCount;
    NTSTATUS                     LastFrameStatus;

    // RTP sequence number and timestamp for media packets
    USHORT                       RtpSeqNum;
    ULONG                        RtpTimestamp;

    // Work item for deferring AVDTP signaling processing to PASSIVE_LEVEL.
    // L2CAP receive callbacks can arrive at DISPATCH_LEVEL — all AVDTP
    // processing (which allocates BRBs via WdfRequestCreate) must run
    // at PASSIVE_LEVEL via this work item.
    WDFWORKITEM              AvdtpWorkItem;
    // AVPTP signaling fragment/reassembly buffer. L2CAP's default MTU is 672;
    // some stacks negotiate a larger MTU, so use the same bounded maximum as
    // the host-tested reassembler rather than silently truncating a packet.
    UCHAR                    AvdtpWorkBuf[OWB_AVDTP_SIGNAL_MAX];
    USHORT                   AvdtpWorkLen;
    OWB_RECV_QUEUE           RecvQueue;
    WDFSPINLOCK              RecvLock;
    LONG                     WorkItemQueued;
    LONG                     ProtocolFault;
    LONG                     ClosingChannels;

    // Timer periódico que (re)abre el canal de señalización cuando el AVDTP está
    // Idle: maneja el link no listo en EvtDeviceAdd y la reconexión tras caída
    // del enlace BT. El callback corre a DISPATCH_LEVEL (un timer PASSIVE no está
    // soportado como hijo de este device) y encola ReconnectWorkItem, que hace el
    // trabajo L2CAP a PASSIVE_LEVEL.
    WDFTIMER                 ReconnectTimer;
    WDFWORKITEM              ReconnectWorkItem;
} OWB_DEVICE_EXTENSION, *POWB_DEVICE_EXTENSION;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(OWB_DEVICE_EXTENSION, OwbGetDeviceExtension)

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD OwbEvtDeviceAdd;
EVT_WDF_WORKITEM OwbAvdtpWorkCallback;
EVT_WDF_WORKITEM OwbReconnectWorkCallback;
EVT_WDF_TIMER    OwbReconnectTimer;
EVT_WDF_DEVICE_D0_EXIT OwbEvtDeviceD0Exit;
EVT_WDF_DEVICE_SELF_MANAGED_IO_CLEANUP OwbEvtDeviceSelfManagedIoCleanup;
