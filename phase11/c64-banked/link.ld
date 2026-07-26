/* Phase 11 stock-C64 reference-operations stopgap.
 *
 * The host loads three initialized segments. The endpoint then disables all
 * maskable C64 interrupts and exposes RAM beneath I/O and KERNAL for the
 * portable package state and high code. No REU is involved.
 */

__basic_zp_start = 0x0002;
__basic_zp_end = 0x0090;
__rc0 = __basic_zp_start;
INCLUDE imag-regs.ld
__basic_zp_size = __basic_zp_end - __basic_zp_start;

MEMORY {
    extra (rx) : ORIGIN = 0x053F, LENGTH = 0x02C2
    main (rwx) : ORIGIN = 0x0801, LENGTH = 0xB7FF
    state (rw) : ORIGIN = 0xC000, LENGTH = 0x21FE
    high (rx)  : ORIGIN = 0xE1FE, LENGTH = 0x1E02
    zp         : ORIGIN = __rc31 + 1, LENGTH = __basic_zp_end - (__rc31 + 1)
}

REGION_ALIAS("c_readonly", main)
REGION_ALIAS("c_writeable", state)
INPUT(basic-header.o)

SECTIONS {
    .basic_header : { *(.basic_header) } >main

    INCLUDE zp.ld

    .extra_text : {
        *(.text.*multiply_unsigned_32)
        *(.text.*phase11_reference_ops_endpoint_c64*stop)
        *(.text.*phase11_reference_ops_endpoint_c64*read32)
        *(.text.unlikely.*slice_index_fail)
        *(.text.unlikely.*panic_fmt)
        *(.text.unlikely.*panic_bounds_check)
    } >extra

    .high_text : {
        *(.text.*phase1013rotate_vector)
        *(.text.*phase1019cross_rate_position)
        *(.text.*phase1018quaternion_product)
        *(.text.*phase11_reference_ops_endpoint_c64*write32)
    } >high

    /* The default C64 charset initializer calls KERNAL CHROUT at $FFD2.
     * This headless target maps KERNAL out in .init.005, so discard it. */
    /DISCARD/ : { *(.init.250) }

    .text : {
        _init = .;
        _start = .;
        KEEP(*(SORT_BY_INIT_PRIORITY(.init.* .init)))
        KEEP(*(.call_main))
        KEEP(*(.after_main))
        _fini = .;
        KEEP(*(SORT_BY_INIT_PRIORITY(.fini.* .fini)))
        KEEP(*(.fini_rts))
        *(.text .text.* CODE LOWCODE ONCE STARTUP)
        __init_array_start = .;
        KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.* .init_array)))
        __init_array_end = .;
        __fini_array_start = .;
        KEEP(*(SORT_BY_INIT_PRIORITY(.fini_array.* .fini_array)))
        __fini_array_end = .;
    } >main

    INCLUDE rodata.ld
    INCLUDE data.ld
    INCLUDE bss.ld
    INCLUDE noinit.ld
}

/* A bounded emergency software-stack area lies between the result block and
 * the extra code segment. Normal Rust frames use .Lstatic_stack in state RAM. */
__stack = ORIGIN(extra);

__extra_load_end = ADDR(.extra_text) + SIZEOF(.extra_text);
__main_load_end = LOADADDR(.data) + SIZEOF(.data);
__high_load_end = ADDR(.high_text) + SIZEOF(.high_text);

ASSERT(SIZEOF(.noinit) == LENGTH(state), "Phase 11 state layout changed");
ASSERT(__extra_load_end <= ORIGIN(extra) + LENGTH(extra), "extra bank overflow");
ASSERT(__main_load_end <= ORIGIN(main) + LENGTH(main), "main bank overflow");
ASSERT(__high_load_end <= ORIGIN(high) + LENGTH(high), "high bank overflow");

OUTPUT_FORMAT {
    BYTE(0x4b) BYTE(0x53) BYTE(0x42) BYTE(0x31)
    SHORT(1)
    SHORT(_start)
    SHORT(ORIGIN(extra)) SHORT(__extra_load_end - ORIGIN(extra))
    SHORT(ORIGIN(main)) SHORT(__main_load_end - ORIGIN(main))
    SHORT(ORIGIN(high)) SHORT(__high_load_end - ORIGIN(high))
    TRIM(extra)
    TRIM(main)
    TRIM(high)
}
