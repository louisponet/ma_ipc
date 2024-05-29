#include <atomic>
#include <cstddef>
#include <cstdint>
enum class FFIError: uint32_t {
	Success = 0,
	UnsupportedMessageSize = 1,
	QueueLengthNotPowerTwo = 2,
	QueueUnInitialized = 3,
	QueueEmpty = 4,
	SpedPast = 5 
};

enum class QueueType: uint8_t {
	Unknown,
	MPMC,
	SPMC
};


struct QueueHeader {
    QueueType queue_type;
    uint8_t elsize_shift_left_bits;
    uint8_t is_initialized;
    uint8_t _pad;
    uint32_t elsize;
    std::size_t mask;
    std::atomic<std::size_t> count;
};


struct Consumer {
    alignas(64) std::size_t  pos;
                std::size_t  mask;
                std::size_t  expected_version;
                std::uint8_t is_running;
                std::uint8_t _pad[7];
                QueueHeader* queue;
                std::size_t  queue_size_in_bytes;
};

struct Producer {
    alignas(64) std::uint8_t  produced_first;
                QueueHeader* queue;
                std::size_t  queue_size_in_bytes;
};

extern "C" {
	FFIError init_consumer120(const char* path, Consumer* consumer);
	FFIError init_producer120(QueueHeader* queue, Producer* consumer);
	FFIError init_consumer56(const char* path, Consumer* consumer);
	FFIError init_producer56(QueueHeader* queue, Producer* consumer);

	FFIError queue_size_in_bytes(uint32_t msgsize_bytes, size_t queue_len, size_t* size_in_bytes );
	FFIError seqlock_size_in_bytes(uint32_t msgsize_bytes, size_t queue_len, size_t* size_in_bytes );

	FFIError open_queue_shmem(const char* path, QueueHeader** header);

	FFIError consume_120(Consumer* consumer, void* msg);
	FFIError produce_120(Producer* producer, void* msg);
	FFIError consume_56(Consumer* consumer, void* msg);
	FFIError produce_56(Producer* producer, void* msg);
}
