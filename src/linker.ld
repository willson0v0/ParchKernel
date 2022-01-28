OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80000000;
PHYS_END_ADDRESS = 0x100000000;
SUPERBLOCK_ADDRESS = 0xFFFFF000;
PAGE_BITMAP_FS_ADDRESS = 0xFFFEF000;
PAGE_BITMAP_MM_ADDRESS = 0xFFFDF000;
INODE_BITMAP_ADDRESS = 0xFFFDE000;
INODE_LIST_ADDRESS = 0xFFDDE000;

SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        *(.text.entry)
        *(.text .text.*)
        . = ALIGN(0x1000);
        strampoline = .;
        *(.text.trampoline);
        *(.text.trampoline_rust);
        . = ALIGN(4K);
        etrampoline = .;
        sutrampoline = .;
        *(.text.u_trampoline);
        *(.text.u_trampoline_rust);
        . = ALIGN(4K);
        eutrampoline = .;
        *(.text .text.*)
    }

    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : {
        *(.data .data.*)
    }

    . = ALIGN(4K);
    edata = .;
    sbss_with_stack = .;
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    .reserve INODE_LIST_ADDRESS : {
        sreserve = .;
        . = PHYS_END_ADDRESS;
    }
    ereserve = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}