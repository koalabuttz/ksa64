# Phase 3 sensor contract

All sensors are deterministic for a fixed nonzero seed. A zero configured seed
is replaced by `0x6d2b79f5`. Noise is centered triangular noise from two bounded
xorshift32 draws, followed by nearest-step quantization.

| Sensor | Rate | Latency | Resolution | Bias | Noise bound |
|---|---:|---:|---:|---:|---:|
| accelerometer axes | 8 Hz | 0 | 0.01 m/s2 | +0.002 m/s2 | +/-0.01 m/s2 |
| pitch gyro | 8 Hz | 0 | 0.01 deg/s | +0.002 deg/s | +/-0.005 deg/s |
| steering feedback | 8 Hz | 0 | binary angle | 0 | 0 |
| clock | 8 Hz | 0 | Q16.16 s | +20 ppm drift | 0 |
| altitude | 4 Hz | 1 step | 10 m | +20 m | +/-10 m |
| GPS-like PVT | 1 Hz | 2 steps | 10 m, 0.1 m/s | 0 | +/-20 m, +/-0.2 m/s |

The altimeter is valid at or below 80 km. GPS-like PVT begins acquisition at
T+120 s. Fault windows suppress validity rather than substituting truth.
Delayed samples use fixed-capacity queues; no allocation is permitted.
