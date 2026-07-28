#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define KPS1_HEADER_LENGTH 48u
#define KPS1_REQUIRED_FLAG_MASK 0xffff0000u
#define KPS1_MAX_PAYLOAD_LENGTH (256u * 1024u)

static uint16_t read_u16(const uint8_t *bytes, size_t offset) {
    return (uint16_t)(bytes[offset] | ((uint16_t)bytes[offset + 1] << 8));
}

static uint32_t read_u32(const uint8_t *bytes, size_t offset) {
    return (uint32_t)bytes[offset]
        | ((uint32_t)bytes[offset + 1] << 8)
        | ((uint32_t)bytes[offset + 2] << 16)
        | ((uint32_t)bytes[offset + 3] << 24);
}

static uint64_t read_u64(const uint8_t *bytes, size_t offset) {
    return (uint64_t)read_u32(bytes, offset) | ((uint64_t)read_u32(bytes, offset + 4) << 32);
}

static uint32_t crc32_update(uint32_t crc, const uint8_t *bytes, size_t length) {
    for (size_t index = 0; index < length; ++index) {
        crc ^= bytes[index];
        for (unsigned bit = 0; bit < 8; ++bit) {
            const uint32_t mask = (uint32_t)-(int32_t)(crc & 1u);
            crc = (crc >> 1) ^ (0xedb88320u & mask);
        }
    }
    return crc;
}

static int validate_kps1(const uint8_t *bytes, size_t length) {
    if (length < KPS1_HEADER_LENGTH || memcmp(bytes, "KPS1", 4) != 0) return 0;
    if (read_u16(bytes, 4) != 1 || read_u16(bytes, 6) != 0 || read_u16(bytes, 8) != KPS1_HEADER_LENGTH) return 0;
    if (read_u16(bytes, 10) == 0 || (read_u32(bytes, 12) & KPS1_REQUIRED_FLAG_MASK) != 0) return 0;
    const uint32_t payload_length = read_u32(bytes, 40);
    if (payload_length > KPS1_MAX_PAYLOAD_LENGTH || length != KPS1_HEADER_LENGTH + payload_length) return 0;
    uint32_t crc = crc32_update(0xffffffffu, bytes, 44);
    crc = crc32_update(crc, bytes + KPS1_HEADER_LENGTH, payload_length) ^ 0xffffffffu;
    return crc == read_u32(bytes, 44);
}

int main(void) {
    static const uint8_t vector[] = {
        0x4b,0x50,0x53,0x31,0x01,0x00,0x00,0x00,0x30,0x00,0x00,0x01,0x04,0x00,0x00,0x00,
        0x08,0x07,0x06,0x05,0x04,0x03,0x02,0x01,0x2a,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
        0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x04,0x00,0x00,0x00,0xa0,0xed,0x0a,0x0c,
        0x10,0x20,0x30,0x40
    };
    if (!validate_kps1(vector, sizeof(vector))) return 1;
    if (read_u16(vector, 10) != 0x0100u || read_u32(vector, 12) != 4u) return 2;
    if (read_u64(vector, 16) != UINT64_C(0x0102030405060708)) return 3;
    if (read_u64(vector, 24) != UINT64_C(42) || read_u64(vector, 32) != 0) return 4;
    uint8_t corrupt[sizeof(vector)];
    memcpy(corrupt, vector, sizeof(vector));
    corrupt[sizeof(corrupt) - 1] ^= 1u;
    if (validate_kps1(corrupt, sizeof(corrupt))) return 5;
    printf("KPS1 C vector passed (%zu bytes)\n", sizeof(vector));
    return 0;
}
