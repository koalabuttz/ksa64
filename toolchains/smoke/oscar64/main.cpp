struct FixedValue {
    long raw;

    constexpr FixedValue(long value) : raw(value) {}
};

static_assert(sizeof(unsigned) == 2, "Oscar64 unsigned must be 16 bits");
static_assert(sizeof(long) == 4, "Oscar64 long must be 32 bits");
static_assert(sizeof(FixedValue) == 4, "Numeric wrapper must add no storage");

int main(void) {
    volatile unsigned char * const border =
        (volatile unsigned char *)0xd020;
    *border = 0;
    return 0;
}

