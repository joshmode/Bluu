// tests/driver/recv_queue_test.cpp
#include <gtest/gtest.h>

#include "recv_queue.h"

TEST(RecvQueue, FifoOrder) {
    OWB_RECV_QUEUE queue{};
    OwbRecvQueueInit(&queue);
    ASSERT_TRUE(OwbRecvQueuePush(&queue, 10u));
    ASSERT_TRUE(OwbRecvQueuePush(&queue, 20u));
    unsigned long length = 0;
    ASSERT_TRUE(OwbRecvQueuePop(&queue, &length));
    EXPECT_EQ(length, 10u);
    ASSERT_TRUE(OwbRecvQueuePop(&queue, &length));
    EXPECT_EQ(length, 20u);
    EXPECT_FALSE(OwbRecvQueuePop(&queue, &length));
}

TEST(RecvQueue, RejectsZeroAndNull) {
    OWB_RECV_QUEUE queue{};
    OwbRecvQueueInit(&queue);
    EXPECT_FALSE(OwbRecvQueuePush(&queue, 0u));
    EXPECT_FALSE(OwbRecvQueuePush(nullptr, 1u));
    unsigned long length = 0;
    EXPECT_FALSE(OwbRecvQueuePop(nullptr, &length));
    EXPECT_FALSE(OwbRecvQueuePop(&queue, nullptr));
    EXPECT_EQ(OwbRecvQueueCount(nullptr), 0u);
}

TEST(RecvQueue, FullQueueSetsOverflow) {
    OWB_RECV_QUEUE queue{};
    OwbRecvQueueInit(&queue);
    for (unsigned long i = 1; i <= OWB_RECV_QUEUE_CAPACITY; ++i)
        ASSERT_TRUE(OwbRecvQueuePush(&queue, i)) << i;
    EXPECT_FALSE(queue.overflow);
    EXPECT_FALSE(OwbRecvQueuePush(&queue, 99u));
    EXPECT_TRUE(queue.overflow);

    unsigned long length = 0;
    ASSERT_TRUE(OwbRecvQueuePop(&queue, &length));
    EXPECT_EQ(length, 1u);
    ASSERT_TRUE(OwbRecvQueuePush(&queue, 99u));
    EXPECT_TRUE(queue.overflow);  // Overflow remains latched until clear.
}

TEST(RecvQueue, ClearResetsCapacityAndOverflow) {
    OWB_RECV_QUEUE queue{};
    OwbRecvQueueInit(&queue);
    for (unsigned long i = 1; i <= OWB_RECV_QUEUE_CAPACITY + 1u; ++i)
        OwbRecvQueuePush(&queue, i);
    EXPECT_TRUE(queue.overflow);
    OwbRecvQueueClear(&queue);
    EXPECT_FALSE(queue.overflow);
    EXPECT_EQ(queue.count, 0u);
    EXPECT_EQ(OwbRecvQueueCount(&queue), 0u);
    EXPECT_TRUE(OwbRecvQueuePush(&queue, 7u));
}
