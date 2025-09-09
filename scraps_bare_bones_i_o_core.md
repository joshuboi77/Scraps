# Scraps — Bare‑bones I/O Core (Core‑8)

The **minimal, silicon‑level surface** Scraps needs to build everything else by itself. Subject‑first names. No external libraries implied.

## Core‑8
| Intrinsic | Purpose | Semantics |
|---|---|---|
| `mem_load(ptr, width)` | Read RAM/MMIO | Volatile when pointer targets device regs; `width ∈ {1,2,4,8}` |
| `mem_store(ptr, val, width)` | Write RAM/MMIO | Volatile for device regs; must not be elided/reordered across `mem_fence()` |
| `mem_fence()` | Order all memory ops | Full barrier: prevents reordering of loads/stores across it (device + CPU-visible) |
| `mem_cmpxchg(ptr, expect, val)` | Atomic RMW | Compare value at `ptr` to `expect`; on match, write `val`. Returns old value. Acquire‑release semantics |
| `int_disable()` | Mask interrupts | Block maskable IRQs for critical sections |
| `int_enable()` | Unmask interrupts | Re‑enable maskable IRQs |
| `cpu_halt()` | Sleep until IRQ | Low‑power halt/wait until an interrupt arrives |
| `time_counter()` / `time_freq()` | Monotonic timing | 64‑bit counter and its frequency (Hz) for precise time math |

## Optional (feature‑gated)
| Intrinsic | Purpose | Notes |
|---|---|---|
| `io_in(port, width)` | x86 port I/O read | **x86‑only**; ARM has no port I/O |
| `io_out(port, val, width)` | x86 port I/O write | Gate behind `arch_x86` feature |

## Rationale
- **Universality:** Devices are reachable via MMIO → `mem_load/store` is enough to talk to NIC/NVMe/GPU/audio/etc.
- **Correctness:** `mem_fence` + `mem_cmpxchg` enable safe concurrency and device queue protocols.
- **Asynchrony:** `int_disable/enable` + `cpu_halt` let you run interrupt‑driven loops without an OS.
- **Timing:** `time_counter/time_freq` provides precision scheduling and backoff without syscalls.

> With only these calls, Scraps can implement allocators, page mapping, drivers, DMA descriptors, schedulers, syscalls, filesystems, and protocols—all in Scraps.

