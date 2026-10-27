// driver/src/recv_queue.h
// Bounded indication queue for coalescing L2CAP receive lengths before the
// PASSIVE_LEVEL AVDTP work item reads them. Host-testable and allocation-free.
#pragma once
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define OWB_RECV_QUEUE_CAPACITY 8u

typedef struct _OWB_RECV_QUEUE {
    unsigned long lengths[OWB_RECV_QUEUE_CAPACITY];
    unsigned int  head;
    unsigned int  tail;
    unsigned int  count;
    int           overflow;
} OWB_RECV_QUEUE, *POWB_RECV_QUEUE;

void OwbRecvQueueInit(POWB_RECV_QUEUE Queue);

// Returns non-zero on success. A full queue sets overflow and rejects the
// indication; the driver must fail closed instead of silently dropping data.
int OwbRecvQueuePush(POWB_RECV_QUEUE Queue, unsigned long Length);

// Returns non-zero when a length was popped.
int OwbRecvQueuePop(POWB_RECV_QUEUE Queue, unsigned long* Length);

void OwbRecvQueueClear(POWB_RECV_QUEUE Queue);
unsigned int OwbRecvQueueCount(const OWB_RECV_QUEUE* Queue);

#ifdef __cplusplus
}
#endif
