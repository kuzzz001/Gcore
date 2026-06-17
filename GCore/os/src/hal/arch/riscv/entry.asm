    .section .text.entry
    .globl _start
_start:
    # Boot hart (hart 0) only: set up stack and call rust_main
    la sp, boot_stack_top
    call rust_main

    .globl _secondary_start
_secondary_start:
    # Secondary hart entry: set up per-hart stack then call rust_secondary
    # tp register already contains the hart ID (set by OpenSBI)
    # Compute per-hart stack pointer: stack_base + hart_id * STACK_SIZE
    # secondary_stacks is the per-hart stack area
    # STACK_SIZE = 4096 * 16 = 65536 = 2^16, use slli instead of mul (no M ext needed)
    slli t1, tp, 16   # hart_id * 65536
    la t0, secondary_stacks
    add sp, t0, t1
    li t0, 65536
    add sp, sp, t0     # sp at top of stack
    call rust_secondary

    .section .bss.stack
    .globl boot_stack
boot_stack:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:

    .section .bss.secondary_stacks
    .globl secondary_stacks
secondary_stacks:
    .space 4096 * 16 * 8   # 8 harts * 64KiB per stack
