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
    # Compute per-hart stack pointer: stack_base + (hart_id + 1) * STACK_SIZE
    # secondary_stacks is the per-hart stack area
    li t0, 65536    # 4096 * 16 = stack size per hart
    mv t1, tp       # hart_id
    addi t1, t1, 0  # hart_id as offset
    mul t1, t1, t0  # hart_id * stack_size
    la t0, secondary_stacks
    add sp, t0, t1
    addi sp, sp, 65536  # sp at top of stack
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
