// driver/src/recv_queue.c
#include "recv_queue.h"

void OwbRecvQueueInit(POWB_RECV_QUEUE queue) {
    if (!queue) return;
    for (size_t i = 0u; i < OWB_RECV_QUEUE_CAPACITY; ++i) queue->lengths[i] = 0u;
    queue->head = 0u;
    queue->tail = 0u;
    queue->count = 0u;
    queue->overflow = 0;
}

int OwbRecvQueuePush(POWB_RECV_QUEUE queue, unsigned long length) {
    if (!queue || length == 0u) return 0;
    if (queue->count >= OWB_RECV_QUEUE_CAPACITY) {
        queue->overflow = 1;
        return 0;
    }
    queue->lengths[queue->tail] = length;
    queue->tail = (queue->tail + 1u) % OWB_RECV_QUEUE_CAPACITY;
    queue->count++;
    return 1;
}

int OwbRecvQueuePop(POWB_RECV_QUEUE queue, unsigned long* length) {
    if (!queue || !length || queue->count == 0u) return 0;
    *length = queue->lengths[queue->head];
    queue->head = (queue->head + 1u) % OWB_RECV_QUEUE_CAPACITY;
    queue->count--;
    return 1;
}

void OwbRecvQueueClear(POWB_RECV_QUEUE queue) {
    if (!queue) return;
    queue->head = 0u;
    queue->tail = 0u;
    queue->count = 0u;
    queue->overflow = 0;
}

unsigned int OwbRecvQueueCount(const OWB_RECV_QUEUE* queue) {
    return queue ? queue->count : 0u;
}
