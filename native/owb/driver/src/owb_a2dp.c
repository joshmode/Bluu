// driver/src/owb_a2dp.c
// GUIDs are instantiated by <initguid.h> in owb_a2dp.h.
#include "owb_a2dp.h"
#include "avdtp.h"
#include "l2cap_stream.h"
#include "ioctl.h"
#include "owb_trace.h"

// DEVPKEY_Bluetooth_DeviceAddress {2BD67D8B-8BEB-48D5-87E0-6CDA3428040A}, 1
// La BT address remota expuesta por el stack BT como string hex de 12 chars
// (p.ej. "88C9E86B1EBF"). DevicePropertyAddress (legacy) viene vacía en los
// nodos BTHENUM de este equipo, así que se usa esta.
DEFINE_DEVPROPKEY(OWB_DEVPKEY_Bluetooth_DeviceAddress,
                  0x2BD67D8B, 0x8BEB, 0x48D5, 0x87, 0xE0, 0x6C, 0xDA,
                  0x34, 0x28, 0x04, 0x0A, 1);

// Convierte un string hex ("88C9E86B1EBF") a BTH_ADDR (48 bits bajos).
static BTH_ADDR OwbParseBtAddrHex(_In_ PCWSTR Str)
{
    BTH_ADDR addr = 0;
    for (PCWSTR p = Str; *p != L'\0'; ++p) {
        WCHAR c = *p;
        BTH_ADDR nibble;
        if      (c >= L'0' && c <= L'9') nibble = (BTH_ADDR)(c - L'0');
        else if (c >= L'a' && c <= L'f') nibble = (BTH_ADDR)(10 + (c - L'a'));
        else if (c >= L'A' && c <= L'F') nibble = (BTH_ADDR)(10 + (c - L'A'));
        else continue;
        addr = (addr << 4) | nibble;
    }
    return addr & 0x0000FFFFFFFFFFFFull;
}

// PASSIVE_LEVEL worker: drains queued AVDTP signaling data via BRB.
// Indications only enqueue lengths at DISPATCH_LEVEL; all BRB work is here.
VOID OwbAvdtpWorkCallback(_In_ WDFWORKITEM WorkItem)
{
    WDFDEVICE device = (WDFDEVICE)WdfWorkItemGetParentObject(WorkItem);
    POWB_DEVICE_EXTENSION ext = OwbGetDeviceExtension(device);

    InterlockedExchange(&ext->WorkItemQueued, 0);
    if (!ext->BthInterface.BthAllocateBrb) return;

    for (;;) {
        unsigned long pending = 0;
        WdfSpinLockAcquire(ext->RecvLock);
        const BOOLEAN have = OwbRecvQueuePop(&ext->RecvQueue, &pending) != 0;
        WdfSpinLockRelease(ext->RecvLock);
        if (!have) break;

        const L2CAP_CHANNEL_HANDLE channel =
            L2capSignalingChannelHandle(ext);
        if (channel == NULL) {
            OwbLog("AvdtpWork: signaling channel closed; dropping queued read");
            break;
        }

        // Request the full indicated length. Truncating it leaves the rest of
        // the L2CAP packet queued and would desynchronize AVDTP framing.
        if (pending > sizeof(ext->AvdtpWorkBuf) ||
            pending > OWB_AVDTP_SIGNAL_MAX) {
            OwbLog("AvdtpWork: signaling packet too large (%lu)", pending);
            InterlockedExchange(&ext->ProtocolFault, 1);
            break;
        }
        const ULONG readLen = pending;
        struct _BRB_L2CA_ACL_TRANSFER* brb =
            (struct _BRB_L2CA_ACL_TRANSFER*)
            ext->BthInterface.BthAllocateBrb(BRB_L2CA_ACL_TRANSFER, 'OWBR');
        if (!brb) {
            OwbLog("AvdtpWork: BthAllocateBrb failed (out of memory)");
            break;
        }

        brb->BtAddress     = ext->RemoteBtAddress;
        brb->ChannelHandle = channel;
        brb->TransferFlags = ACL_TRANSFER_DIRECTION_IN | ACL_SHORT_TRANSFER_OK;
        brb->BufferSize    = readLen;
        brb->Buffer        = ext->AvdtpWorkBuf;
        brb->BufferMDL     = NULL;

        const NTSTATUS status = L2capSubmitBrb(ext, (PBRB)brb, sizeof(*brb));
        if (NT_SUCCESS(status) && brb->BufferSize > 0) {
            ext->AvdtpWorkLen = (USHORT)brb->BufferSize;
            AvdtpHandleSignalingPacket(ext, ext->AvdtpWorkBuf, ext->AvdtpWorkLen);
            ext->AvdtpWorkLen = 0;
        } else {
            OwbLog("AvdtpWork: BRB IN failed 0x%x (size=%lu)",
                   status, brb->BufferSize);
            ext->BthInterface.BthFreeBrb((PBRB)brb);
            break;
        }
        ext->BthInterface.BthFreeBrb((PBRB)brb);
    }
}

// Período del timer de reconexión.
#define OWB_RECONNECT_PERIOD_MS 3000u

// TRUE si conviene (re)intentar abrir el canal de señalización: interfaz BT
// lista, dirección conocida, sin canal abierto y AVDTP Idle.
static BOOLEAN OwbShouldReconnect(_In_ POWB_DEVICE_EXTENSION ext)
{
    return ext->BthInterface.BthAllocateBrb != NULL &&
           ext->RemoteBtAddress != 0 &&
           L2capSignalingChannelHandle(ext) == NULL &&
           OwbAvdtpStateCanReconnect(&ext->Avdtp) &&
           ext->ProtocolFault == 0;
}

VOID OwbEvtDeviceD0Exit(_In_ WDFDEVICE Device, _In_ WDF_POWER_DEVICE_STATE TargetState) {
    UNREFERENCED_PARAMETER(TargetState);
    POWB_DEVICE_EXTENSION ext = OwbGetDeviceExtension(Device);
    // WdfTimerStop(TRUE) waits for the timer callback and is not valid in a
    // power callback at elevated IRQL. Stop without waiting; parenting the timer
    // to the device guarantees it cannot outlive this extension.
    WdfTimerStop(ext->ReconnectTimer, FALSE);
    L2capPrepareCloseChannels(ext);
    ext->Avdtp.State = OwbAvdtpStateIdle;
    ext->Avdtp.ResponsePending = 0;
}

VOID OwbEvtDeviceSelfManagedIoCleanup(_In_ WDFDEVICE Device) {
    POWB_DEVICE_EXTENSION ext = OwbGetDeviceExtension(Device);
    L2capPrepareCloseChannels(ext);
}

// DISPATCH_LEVEL: el callback del timer solo mira el estado y, si toca reconectar,
// encola el work item (no se puede tocar L2CAP/BRBs a DISPATCH_LEVEL).
VOID OwbReconnectTimer(_In_ WDFTIMER Timer)
{
    WDFDEVICE device = (WDFDEVICE)WdfTimerGetParentObject(Timer);
    POWB_DEVICE_EXTENSION ext = OwbGetDeviceExtension(device);
    WdfWorkItemEnqueue(ext->ReconnectWorkItem);
}

// PASSIVE_LEVEL: reintenta abrir el canal de señalización (usa BRBs).
VOID OwbReconnectWorkCallback(_In_ WDFWORKITEM WorkItem)
{
    WDFDEVICE device = (WDFDEVICE)WdfWorkItemGetParentObject(WorkItem);
    POWB_DEVICE_EXTENSION ext = OwbGetDeviceExtension(device);
    AvdtpHandleTick(ext);
    L2capClearForReconnect(ext);
    if (!OwbShouldReconnect(ext)) return;

    OwbLog("Reconnect: AVDTP Idle -> reintentando abrir signaling");
    (VOID)L2capOpenSignalingChannel(ext);
}

NTSTATUS
DriverEntry(
    _In_ PDRIVER_OBJECT  DriverObject,
    _In_ PUNICODE_STRING RegistryPath)
{
    WDF_DRIVER_CONFIG config;
    NTSTATUS          status;

    WDF_DRIVER_CONFIG_INIT(&config, OwbEvtDeviceAdd);

    status = WdfDriverCreate(DriverObject, RegistryPath,
                             WDF_NO_OBJECT_ATTRIBUTES, &config, WDF_NO_HANDLE);
    if (!NT_SUCCESS(status)) {
        OwbLog("WdfDriverCreate fallo 0x%x", status);
        return status;
    }

    OwbLog("driver cargado v%d.%d",
           OWB_DRIVER_VERSION_MAJOR, OWB_DRIVER_VERSION_MINOR);
    return STATUS_SUCCESS;
}

NTSTATUS
OwbEvtDeviceAdd(
    _In_    WDFDRIVER       Driver,
    _Inout_ PWDFDEVICE_INIT DeviceInit)
{
    NTSTATUS              status;
    WDFDEVICE             device;
    WDF_OBJECT_ATTRIBUTES attributes;
    POWB_DEVICE_EXTENSION ext;

    UNREFERENCED_PARAMETER(Driver);

    OwbLog("EvtDeviceAdd: inicio");

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, OWB_DEVICE_EXTENSION);
    WDF_PNPPOWER_EVENT_CALLBACKS pnpCallbacks;
    WDF_PNPPOWER_EVENT_CALLBACKS_INIT(&pnpCallbacks);
    pnpCallbacks.EvtDeviceD0Exit = OwbEvtDeviceD0Exit;
    pnpCallbacks.EvtDeviceSelfManagedIoCleanup = OwbEvtDeviceSelfManagedIoCleanup;
    WdfDeviceInitSetPnpPowerEventCallbacks(DeviceInit, &pnpCallbacks);

    // Seguridad del device: la deja PnP por defecto (SYSTEM full; el servicio
    // corre como LocalSystem y abre \\.\OpenWinBlue sin problema). NO se usa
    // WdfDeviceInitAssignSDDLString aquí: es para control devices, y en un PnP
    // FDO hacía fallar WdfDeviceCreate con STATUS_INVALID_SECURITY_DESCR
    // (0xC0000079). El endurecimiento SYS+Admin se hará por el INF [DDInstall.HW].
    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status)) {
        OwbLog("WdfDeviceCreate fallo 0x%x", status);
        return status;
    }
    OwbLog("EvtDeviceAdd: WDFDEVICE creado");

    ext = OwbGetDeviceExtension(device);
    RtlZeroMemory(ext, sizeof(*ext));
    ext->Device      = device;
    ext->BthIoTarget = WdfDeviceGetIoTarget(device);
    ext->SampleRateHz = 44100u;
    AvdtpContextInit(&ext->Avdtp);

    // BT address remota vía DEVPKEY_Bluetooth_DeviceAddress (string hex).
    WDF_DEVICE_PROPERTY_DATA propData;
    WDF_DEVICE_PROPERTY_DATA_INIT(&propData, &OWB_DEVPKEY_Bluetooth_DeviceAddress);

    WCHAR       addrBuf[32] = { 0 };
    ULONG       reqLen   = 0;
    DEVPROPTYPE propType = 0;
    status = WdfDeviceQueryPropertyEx(device, &propData,
                                      sizeof(addrBuf), addrBuf, &reqLen, &propType);
    if (NT_SUCCESS(status) && propType == DEVPROP_TYPE_STRING) {
        ext->RemoteBtAddress = OwbParseBtAddrHex(addrBuf);
        OwbLog("EvtDeviceAdd: BT addr remota %012I64x", ext->RemoteBtAddress);
    } else {
        OwbLog("EvtDeviceAdd: query BT addr fallo 0x%x type=%lu - continuo",
               status, propType);
    }
    status = STATUS_SUCCESS;

    // Acquire BthPort profile driver interface.
    // Version 0x0200 (BTHDDI_PROFILE_DRIVER_INTERFACE_VERSION_FOR_QI) per bthddi.h.
    status = WdfFdoQueryForInterface(device,
                                     &GUID_BTHDDI_PROFILE_DRIVER_INTERFACE,
                                     (PINTERFACE)&ext->BthInterface,
                                     sizeof(BTH_PROFILE_DRIVER_INTERFACE),
                                     BTHDDI_PROFILE_DRIVER_INTERFACE_VERSION_FOR_QI,
                                     NULL);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: QI BthPort DDI fallo 0x%x - continuo SIN interfaz BT", status);
        status = STATUS_SUCCESS;
    } else {
        OwbLog("EvtDeviceAdd: interfaz BthPort DDI adquirida");
    }

    // Create symbolic link \\.\OpenWinBlue for user-mode DeviceIoControl.
    DECLARE_CONST_UNICODE_STRING(symLink, L"\\DosDevices\\OpenWinBlue");
    status = WdfDeviceCreateSymbolicLink(device, &symLink);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: symlink DosDevices fallo 0x%x", status);
        return status;
    }
    OwbLog("EvtDeviceAdd: symlink \\\\.\\OpenWinBlue creado");

    // Register IOCTL dispatch queue.
    status = IoctlRegister(device);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: IoctlRegister fallo 0x%x", status);
        return status;
    }

    // Create the AVDTP deferred work item.
    // Worker runs at PASSIVE_LEVEL — safe to call WdfRequestCreate.
    WDF_WORKITEM_CONFIG workCfg;
    WDF_WORKITEM_CONFIG_INIT(&workCfg, OwbAvdtpWorkCallback);
    WDF_OBJECT_ATTRIBUTES workAttribs;
    WDF_OBJECT_ATTRIBUTES_INIT(&workAttribs);
    workAttribs.ParentObject = device;
    status = WdfWorkItemCreate(&workCfg, &workAttribs, &ext->AvdtpWorkItem);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: WdfWorkItemCreate fallo 0x%x", status);
        return status;
    }

    // Create a pre-allocated WDFREQUEST for synchronous BRB submission.
    // Reused across all BRB calls via WdfRequestReuse (see L2capSubmitBrb).
    WDF_OBJECT_ATTRIBUTES reqAttribs;
    WDF_OBJECT_ATTRIBUTES_INIT(&reqAttribs);
    reqAttribs.ParentObject = device;
    status = WdfRequestCreate(&reqAttribs, ext->BthIoTarget, &ext->BrbRequest);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: WdfRequestCreate (BrbRequest) fallo 0x%x", status);
        return status;
    }

    // Lock que serializa el uso del BrbRequest compartido (ver L2capSubmitBrb).
    WDF_OBJECT_ATTRIBUTES lockAttribs;
    WDF_OBJECT_ATTRIBUTES_INIT(&lockAttribs);
    lockAttribs.ParentObject = device;
    status = WdfWaitLockCreate(&lockAttribs, &ext->BrbLock);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: WdfWaitLockCreate fallo 0x%x", status);
        return status;
    }

    // Work item de reconexión (PASSIVE) que abre el canal de señalización.
    WDF_OBJECT_ATTRIBUTES spinAttribs;
    WDF_OBJECT_ATTRIBUTES_INIT(&spinAttribs);
    spinAttribs.ParentObject = device;
    status = WdfSpinLockCreate(&spinAttribs, &ext->RecvLock);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: WdfSpinLockCreate (RecvLock) fallo 0x%x", status);
        return status;
    }
    OwbRecvQueueInit(&ext->RecvQueue);

    WDF_WORKITEM_CONFIG reconnCfg;
    WDF_WORKITEM_CONFIG_INIT(&reconnCfg, OwbReconnectWorkCallback);
    WDF_OBJECT_ATTRIBUTES reconnAttribs;
    WDF_OBJECT_ATTRIBUTES_INIT(&reconnAttribs);
    reconnAttribs.ParentObject = device;
    status = WdfWorkItemCreate(&reconnCfg, &reconnAttribs, &ext->ReconnectWorkItem);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: WdfWorkItemCreate (reconnect) fallo 0x%x", status);
        return status;
    }

    // Timer periódico (DISPATCH) que dispara el work item de reconexión mientras
    // el AVDTP esté Idle. Maneja link no listo al arranque y reconexión.
    WDF_TIMER_CONFIG timerCfg;
    WDF_TIMER_CONFIG_INIT_PERIODIC(&timerCfg, OwbReconnectTimer, OWB_RECONNECT_PERIOD_MS);
    WDF_OBJECT_ATTRIBUTES timerAttribs;
    WDF_OBJECT_ATTRIBUTES_INIT(&timerAttribs);
    timerAttribs.ParentObject = device;
    status = WdfTimerCreate(&timerCfg, &timerAttribs, &ext->ReconnectTimer);
    if (!NT_SUCCESS(status)) {
        OwbLog("EvtDeviceAdd: WdfTimerCreate fallo 0x%x", status);
        return status;
    }

    // Abrir el canal de señalización AVDTP es BEST-EFFORT: es una tarea de
    // runtime, no un requisito para crear el device. Su fallo (link BT no listo,
    // RemoteBtAddress==0, BthInterface aún no adquirida, etc.) NO debe hacer
    // fallar EvtDeviceAdd — si no, WDF destruye el device y desaparece el
    // control interface \\.\OpenWinBlue, y el servicio nunca puede conectar ni
    // disparar la negociación. La conexión se reintenta vía ReconnectTimer.
    NTSTATUS l2capStatus = L2capOpenSignalingChannel(ext);
    OwbLog("EvtDeviceAdd: L2capOpenSignalingChannel -> 0x%x (best-effort)", l2capStatus);

    WdfTimerStart(ext->ReconnectTimer, WDF_REL_TIMEOUT_IN_MS(OWB_RECONNECT_PERIOD_MS));

    OwbLog("EvtDeviceAdd: dispositivo agregado v%d.%d OK",
           OWB_DRIVER_VERSION_MAJOR, OWB_DRIVER_VERSION_MINOR);
    return STATUS_SUCCESS;
}
