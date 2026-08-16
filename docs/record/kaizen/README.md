# Kaizen

> Six-step continuous-improvement write-ups: improvement potential → current
> method analysis → ideas → test plan → implementation → evaluation. One file
> per loop. Where a postmortem records how an incident was debugged, a kaizen
> records how a recurring cost was measured, attacked, and re-measured — and
> which standard changed so it stays fixed.
>
> Docs index: [`../README.md`](../../README.md). Project overview:
> [`../../README.md`](../../../README.md). Sibling write-ups:
> [`../postmortems/`](../postmortems/README.md) ·
> [`../tradeoff-curves/`](../tradeoff-curves/README.md).

| Kaizen                                                                                                                                                                                                                         | Status            |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------- |
| [`real-repo-sync.md`](real-repo-sync.md) — push (then `:sync`, now `:gs`) never completes on the real notes repo: ∞ (device bricks) → 24.1 s measured, via the O(depth) TreeBuilder splice + a second localization loop on the push half. | Closed 2026-07-13 |
| [`cold-boot-time.md`](cold-boot-time.md) — cold boot latency, power-on to usable cursor: 4159 → 3239 ms (−22%) from pre-app overhead. Target ≤ 3000 ms not met; the ~2.2 s splash waveform is the remaining floor.                       | Closed, target missed |
