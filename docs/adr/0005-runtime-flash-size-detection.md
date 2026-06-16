# Runtime flash-size detection via a PICOBOOT EXEC stub

## Status

Accepted (2026-06-17)

Refines the flash-address guard from [ADR 0001](0001-scope-flasher-first-rp2040-only.md),
which centralized flash geometry but left the guard's upper bound at a conservative,
hardware-maximum 16 MiB.

## Context

The flash-address guard in `uf2::parse` refused only writes outside the 16 MiB XIP window
(`FLASH_END`). The RP2040 has no on-chip flash; the real size is set by the board's external
QSPI part (commonly 2 MiB), so the 16 MiB bound let an oversized image for a small-flash
board through. Issue #4 asks the guard to reflect the real capacity.

RP2040 PICOBOOT has no command that reports flash size or JEDEC id (datasheet §2.8.5.4,
Table 174; the GET_INFO command exists only in the RP2350 PICOBOOT v2). The only way to learn
the size at runtime is to read the flash's JEDEC ID (RDID, `0x9F`) over the QSPI/SSI
interface, which requires running code on the device — there is no host-side path to the SSI.

PICOBOOT does provide EXEC (`0x08`, §2.8.5.4.8): it calls a function previously placed in RAM
by a WRITE. The function takes no arguments and returns nothing, so it must communicate
through RAM. EXIT_XIP (§2.8.5.4.6) already initialises the SSI for serial transfers and runs
the flash's XIP-exit sequence (§2.8.1.2), leaving the part ready for standard SPI commands.

## Decision

We will detect flash size at runtime by running a small stub on the device that reads the
JEDEC ID, and use the result to tighten the guard.

- **Read the JEDEC ID with a clean-room SSI stub.** `constants::FLASH_ID_STUB` is ARMv6-M
  thumb machine code that drives the Synopsys SSI directly (XIP_SSI base `0x1800_0000`,
  datasheet §4.10.12): it disables the SSI, sets CTRLR0 to 8-bit standard-SPI
  transmit-and-receive, enables it, selects slave 0, pushes `0x9F` plus three dummy bytes,
  spins until `RXFLR == 4`, and reads the four frames (the first is the opcode echo,
  discarded). It stores the three id bytes at a fixed RAM address. The stub is written from
  the public SSI register spec, not from picotool or the pico-sdk boot2
  ([ADR 0003](0003-clean-room-reimplementation-and-license.md)); its assembly source and
  disassembly are reproduced below.
- **Drive it from `Device::detect_flash_size`.** Take exclusive access, EXIT_XIP, WRITE the
  stub to SRAM, EXEC it, and READ back the id. `detect::flash_size_from_jedec` decodes the
  capacity byte `N` to `1 << N`. The stub only reads flash, so the subsequent `load` re-runs
  EXIT_XIP from a clean SSI state.
- **Detection is best-effort; the guard is always on.** An unreadable or implausible id
  (outside `[64 KiB, 16 MiB]`), or any transport error during detection, logs a warning and
  falls back to the conservative `FLASH_END`. A failed query never refuses an otherwise valid
  flash, matching the project's "safety defaults always on" convention.
- **Parameterize the guard.** `uf2::parse` takes a `flash_end` bound instead of hardcoding
  `FLASH_END`, so the caller passes the detected size (or the fallback). The flow becomes
  open → detect → parse → load.
- **Place the stub at main-SRAM base.** See the SRAM caveat under Consequences.

## Considered Options

- **A manual `--flash-size` flag** — rejected: the user asked for automatic detection, and a
  flag adds CLI surface and a footgun (a wrong value silently narrows or widens the guard).
  The fallback already covers the case where detection cannot run.
- **Refuse to flash when detection fails** — rejected: the stub is only verifiable on
  hardware (see below), so a regression in it would block every flash on a perfectly good
  board. Falling back to the always-on conservative guard is safe and keeps the tool usable.
- **A GET_INFO-style query** — not available: that command is RP2350 PICOBOOT v2 only; the
  RP2040 ROM does not implement it (datasheet §2.8.5.4, Table 174).
- **Probe by reading XIP addresses and detecting wrap-around** — rejected: slow, needs XIP
  remapping, and is unreliable (data can coincidentally match across a wrap).

## Consequences

- The flash-address guard reflects the real capacity on boards whose flash answers RDID; an
  oversized image for a 2 MiB board is now refused rather than silently mishandled.
- **The stub is only verifiable on real hardware.** Unit tests cover the EXEC encoding, the
  JEDEC decode, the parameterized guard, and that the stub and result buffer do not overlap;
  the stub's actual execution and the detected value are a manual hardware check, tracked by
  #3. Until then the fallback is the safety net.
- **The stub's SRAM location is not pinned by the datasheet.** The RP2040 ROM does not
  document which SRAM it touches while servicing PICOBOOT, so no address is provably free. We
  use main-SRAM base (`0x2000_0000`): the ROM loads and runs RAM-only UF2 images from the
  lowest main-SRAM address (datasheet §2.8, erratum RP2040-E9), so it must leave that region
  for downloaded code. This choice is confirmed on hardware as part of #3.
- No new dependencies: the stub is a committed byte vector, assembled once from the source
  below, so the build still needs no ARM toolchain.

## Appendix: flash-id stub

Assembled with `llvm-mc -triple=thumbv6m-none-eabi`; the byte vector in
`constants::FLASH_ID_STUB` is the resulting `.text` section.

```asm
    .syntax unified
    .cpu cortex-m0plus
    .thumb
    .thumb_func
    .global flash_id_stub
flash_id_stub:
    push    {r4, lr}            @ EABI: preserve r4, save return address
    movs    r0, #0x18
    lsls    r0, r0, #24         @ r0 = 0x18000000 (XIP_SSI base)
    movs    r1, #0              @ r1 = 0 (zero / dummy byte)
    str     r1, [r0, #0x08]     @ SSIENR = 0 (disable to reconfigure)
    movs    r2, #7
    lsls    r2, r2, #16         @ r2 = 0x00070000
    str     r2, [r0, #0x00]     @ CTRLR0: DFS_32=7 (8-bit), TMOD=0 (tx&rx), FRF=0 (std SPI)
    str     r1, [r0, #0x04]     @ CTRLR1 = 0
    movs    r2, #1
    str     r2, [r0, #0x08]     @ SSIENR = 1 (enable)
    str     r2, [r0, #0x10]     @ SER = 1 (select slave 0)
    movs    r2, #0x9f
    str     r2, [r0, #0x60]     @ DR0 = 0x9F (RDID opcode)
    str     r1, [r0, #0x60]     @ DR0 = 0 (dummy 1)
    str     r1, [r0, #0x60]     @ DR0 = 0 (dummy 2)
    str     r1, [r0, #0x60]     @ DR0 = 0 (dummy 3)
.Lwait:
    ldr     r2, [r0, #0x24]     @ r2 = RXFLR (received frame count)
    cmp     r2, #4
    bcc     .Lwait              @ spin until all 4 frames are received
    movs    r4, #0x20
    lsls    r4, r4, #24
    adds    r4, #0x80           @ r4 = 0x20000080 (RESULT_ADDR)
    ldr     r2, [r0, #0x60]     @ frame 0: opcode echo (discard)
    ldr     r2, [r0, #0x60]     @ frame 1: manufacturer id
    strb    r2, [r4, #0]
    ldr     r2, [r0, #0x60]     @ frame 2: memory type
    strb    r2, [r4, #1]
    ldr     r2, [r0, #0x60]     @ frame 3: capacity
    strb    r2, [r4, #2]
    pop     {r4, pc}            @ return
```

Disassembly (`llvm-objdump -d`):

```
00000000 <flash_id_stub>:
       0: b510          push    {r4, lr}
       2: 2018          movs    r0, #0x18
       4: 0600          lsls    r0, r0, #0x18
       6: 2100          movs    r1, #0x0
       8: 6081          str     r1, [r0, #0x8]
       a: 2207          movs    r2, #0x7
       c: 0412          lsls    r2, r2, #0x10
       e: 6002          str     r2, [r0]
      10: 6041          str     r1, [r0, #0x4]
      12: 2201          movs    r2, #0x1
      14: 6082          str     r2, [r0, #0x8]
      16: 6102          str     r2, [r0, #0x10]
      18: 229f          movs    r2, #0x9f
      1a: 6602          str     r2, [r0, #0x60]
      1c: 6601          str     r1, [r0, #0x60]
      1e: 6601          str     r1, [r0, #0x60]
      20: 6601          str     r1, [r0, #0x60]
      22: 6a42          ldr     r2, [r0, #0x24]
      24: 2a04          cmp     r2, #0x4
      26: d3fc          blo     0x22 <flash_id_stub+0x22>
      28: 2420          movs    r4, #0x20
      2a: 0624          lsls    r4, r4, #0x18
      2c: 3480          adds    r4, #0x80
      2e: 6e02          ldr     r2, [r0, #0x60]
      30: 6e02          ldr     r2, [r0, #0x60]
      32: 7022          strb    r2, [r4]
      34: 6e02          ldr     r2, [r0, #0x60]
      36: 7062          strb    r2, [r4, #0x1]
      38: 6e02          ldr     r2, [r0, #0x60]
      3a: 70a2          strb    r2, [r4, #0x2]
      3c: bd10          pop     {r4, pc}
```
