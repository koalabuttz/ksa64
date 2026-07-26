# Phase 11 prediction reference

`prediction_vectors.py` is an independent float64 implementation of the
bounded central-plus-J2 midpoint-RK2 model used by the Phase 11 host
prediction service. It does not import KSA64 code. The stored vector covers a
100 km inertial state at 0, 1, 5, and 15 seconds.

Normal tests consume the frozen values and do not need Python. The completion
audit runs `python -B phase11/reference/prediction_vectors.py --check` to prove
the fixture still matches the independent generator.

The comparison validates numerical implementation of the declared prediction
model. It does not validate the authoritative Phase 10 world or real-vehicle
accuracy.
