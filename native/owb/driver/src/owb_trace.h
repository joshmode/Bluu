// driver/src/owb_trace.h
//
// Trazas siempre activas del driver. KdPrint desaparece en builds release;
// DbgPrintEx en DPFLTR_ERROR_LEVEL pasa el filtro por defecto de Windows,
// así que se ve en DebugView (Capture Kernel) y WinDbg sin tocar el registro.
//
#pragma once
#include <ntddk.h>

// DPFLTR_DEFAULT_ID + DPFLTR_ERROR_LEVEL es visible en DebugView por defecto,
// sin tocar el registro ni reiniciar (el componente IHVDRIVER exige que el
// filtro se cargue en el boot). Prefijo "OWB:" para filtrar.
#define OwbLog(fmt, ...) \
    DbgPrintEx(DPFLTR_DEFAULT_ID, DPFLTR_ERROR_LEVEL, \
               "OWB: " fmt "\n", ##__VA_ARGS__)

// State names come from the host-tested AVDTP state module (avdtp_state.h).
