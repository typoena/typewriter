# Tradeoff curves

> Where a design knob has a cost that bends — energy, latency, memory — against
> an interval or size, the curve and its knee live here, so the chosen default
> is traceable to a shape rather than a guess.
>
> Docs index: [`../README.md`](../README.md). Project overview:
> [`../../README.md`](../../README.md).

| Curve | What it decides |
| --- | --- |
| [`wifi-auto-sync.md`](wifi-auto-sync.md) | `auto_sync` interval vs Wi-Fi energy (a `1/T` hyperbola) — why the default is 10 min and opportunistic, not a wall-clock timer. |
| [`epd-refresh-latency.md`](epd-refresh-latency.md) | E-ink refresh latency vs rows driven — the full / full-area-partial / windowed-Y cost model behind typing responsiveness and the boot splash→editor swap. |
