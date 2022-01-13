OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80000000;
PHYS_END_ADDRESS = 0x100000000;
SUPERBLOCK_ADDRESS = 0xFFFFF000;
PAGE_BITMAP_ADDRESS = 0xFFFFF000;
INODE_BITMAP_ADDRESS = 0xFFFFF000;

SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        *(.text.entry)
        *(.text .text.*)
        . = ALIGN(0x1000);
        _trampoline = .;
        *(trampsec)
        . = ALIGN(0x1000);
        ASSERT(. - _trampoline == 0x1000, "error: trampoline larger than one page");
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
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    . = INODE_BITMAP_ADDRESS;
    sreserve = .;
    .reserve : {
        sinodebmap = .;
        *(.reserve.inode)
        ASSERT(. == PAGE_BITMAP_ADDRESS, "error: inode bitmap size malformed");
        spagebmap = .;
        *(.reserve.page)
        ASSERT(. == SUPERBLOCK_ADDRESS == 65536, "error: page bitmap size malformed");
        ssuperblock = .;
        *(.reserve.superblock)
        ASSERT(. == PHYS_END_ADDRESS, "error: superblock size malformed");
    }
    ereserve = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}