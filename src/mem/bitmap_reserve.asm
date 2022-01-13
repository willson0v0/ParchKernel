    .section .reserve.inode
    .globl inode_bitmap_reserve
inode_bitmap_reserve:
    .space 4096 * 512
    .globl inode_bitmap_reserve_top
inode_bitmap_reserve_top:

    .section .reserve.page
    .globl page_bitmap_reserve
page_bitmap_reserve:
    .space 4096 * 16
    .globl page_bitmap_reserve_top
page_bitmap_reserve_top:

    .section .reserve.superblock
    .globl superblock_reserve
superblock_reserve:
    .space 4096 * 1
    .globl superblock_reserve_top
superblock_reserve_top:

