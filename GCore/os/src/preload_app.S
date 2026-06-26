    .section .data
    .global sinitproc
    .global einitproc
    .align 12
sinitproc:
    .incbin "../user/target/loongarch64-unknown-linux-gnu/release/initproc"
einitproc:
    .align 12

    .section .data
    .global sbash
    .global ebash
    .align 12
sbash:
    .incbin "./bash-la"
ebash:
    .align 12

    .section .data
    .global sbusybox
    .global ebusybox
    .align 12
sbusybox:
    .incbin "./busybox-la"
ebusybox:
    .align 12

    .section .data
    .global sldmusl
    .global eldmusl
    .align 12
sldmusl:
    .incbin "./ld-musl-loongarch-lp64d.so.1"
eldmusl:
    .align 12

    .section .data
    .global sldglibc
    .global eldglibc
    .align 12
sldglibc:
    .incbin "./ld-linux-loongarch-lp64d.so.1"
eldglibc:
    .align 12
