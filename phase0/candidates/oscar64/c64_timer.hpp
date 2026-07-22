#ifndef KSA64_PHASE0_C64_TIMER_HPP
#define KSA64_PHASE0_C64_TIMER_HPP

void prepare_cia_timing(void);
void start_cia_timer(void);
unsigned long stop_cia_timer(void);
unsigned long measure_cia_boundary_overhead(void);

#endif
