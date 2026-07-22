#include "c64_timer.hpp"

static volatile unsigned char * const CIA1_TIMER_A_LOW =
    (volatile unsigned char *)0xdc04;
static volatile unsigned char * const CIA1_TIMER_A_HIGH =
    (volatile unsigned char *)0xdc05;
static volatile unsigned char * const CIA1_TIMER_B_LOW =
    (volatile unsigned char *)0xdc06;
static volatile unsigned char * const CIA1_TIMER_B_HIGH =
    (volatile unsigned char *)0xdc07;
static volatile unsigned char * const CIA1_INTERRUPT_CONTROL =
    (volatile unsigned char *)0xdc0d;
static volatile unsigned char * const CIA1_CONTROL_A =
    (volatile unsigned char *)0xdc0e;
static volatile unsigned char * const CIA1_CONTROL_B =
    (volatile unsigned char *)0xdc0f;
static volatile unsigned char * const CIA2_INTERRUPT_CONTROL =
    (volatile unsigned char *)0xdd0d;
static volatile unsigned char * const VIC_CONTROL_1 =
    (volatile unsigned char *)0xd011;
static volatile unsigned char * const VIC_RASTER =
    (volatile unsigned char *)0xd012;
static volatile unsigned char * const VIC_SPRITE_ENABLE =
    (volatile unsigned char *)0xd015;

static const unsigned char CONTROL_FORCE_LOAD = 0x10;
static const unsigned char CONTROL_START = 0x01;
static const unsigned char TIMER_B_COUNTS_TIMER_A = 0x40;

static void wait_for_frame_start(void) {
    while (*VIC_RASTER == 0 && (*VIC_CONTROL_1 & 0x80) == 0) {
    }
    while (*VIC_RASTER != 0 || (*VIC_CONTROL_1 & 0x80) != 0) {
    }
}

void prepare_cia_timing(void) {
    *CIA1_INTERRUPT_CONTROL = 0x7f;
    (void)*CIA1_INTERRUPT_CONTROL;
    *CIA2_INTERRUPT_CONTROL = 0x7f;
    (void)*CIA2_INTERRUPT_CONTROL;
    *VIC_CONTROL_1 &= 0xef;
    *VIC_SPRITE_ENABLE = 0x00;
}

void start_cia_timer(void) {
    wait_for_frame_start();
    *CIA1_CONTROL_A = 0x00;
    *CIA1_CONTROL_B = TIMER_B_COUNTS_TIMER_A;

    *CIA1_TIMER_A_LOW = 0xff;
    *CIA1_TIMER_A_HIGH = 0xff;
    *CIA1_TIMER_B_LOW = 0xff;
    *CIA1_TIMER_B_HIGH = 0xff;

    *CIA1_CONTROL_A = CONTROL_FORCE_LOAD;
    *CIA1_CONTROL_B = TIMER_B_COUNTS_TIMER_A | CONTROL_FORCE_LOAD;
    *CIA1_CONTROL_B = TIMER_B_COUNTS_TIMER_A | CONTROL_START;
    *CIA1_CONTROL_A = CONTROL_START;
}

unsigned long stop_cia_timer(void) {
    *CIA1_CONTROL_A = 0x00;
    *CIA1_CONTROL_B = TIMER_B_COUNTS_TIMER_A;

    unsigned long remaining = *CIA1_TIMER_A_LOW;
    remaining |= (unsigned long)*CIA1_TIMER_A_HIGH << 8;
    remaining |= (unsigned long)*CIA1_TIMER_B_LOW << 16;
    remaining |= (unsigned long)*CIA1_TIMER_B_HIGH << 24;
    return 0xffffffffUL - remaining;
}

unsigned long measure_cia_boundary_overhead(void) {
    start_cia_timer();
    return stop_cia_timer();
}
