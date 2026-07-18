# Sealed Split Gates

Recorded before opening either sealed result. Because sealed incumbent evaluations were intentionally not exposed during development, validation and held-out use absolute gates derived from the frozen incumbent development rate.

Validation passes only if all 56 roots remain in the denominator, errors/illegal actions/timeouts are zero, wins are at least 52 (the ceiling of the incumbent development win rate, 115/124), losses are at most one, and p95 remains below the fixed 10-second collection timeout.

Held-out passes only if all 23 roots remain in the denominator, errors/illegal actions/timeouts are zero, wins are at least 22 (the ceiling of 23 times 115/124), losses are at most one, and p95 remains below the fixed 10-second collection timeout.

These thresholds are not changed after observing either result. Any failure stops promotion without tuning.
