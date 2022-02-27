

use core::cell::RefCell;

use alloc::{vec::Vec, boxed::Box};


use super::PhysAddr;

extern "C" {
    fn PAGE_BITMAP_MM_ADDRESS();
    fn PAGE_BITMAP_FS_ADDRESS();
}

// lazy_static! {
//     pub static ref INODE_BITMAP: SpinMutex<BitMap> = 
//         SpinMutex::new("InodeBitmap", BitMap::new((INODE_BITMAP_ADDRESS as usize).into(), INODE_BITMAP_ADDRESS as usize - PAGE_BITMAP_MM_ADDRESS as usize));
// }

pub trait BitMapTrait {
    fn get(&self) -> bool;
    // fn set(&self)
}

pub struct BitMapIndex {
    bits: u64,
    level: usize,
    length: usize,
    sub_entries: Vec<RefCell<BitMapIndex>>,
}

impl BitMapIndex {
    pub fn new(length: usize) -> Self {
        let length = length / 64;
        let mut level = None;
        for i in 0..10 {
            if Self::powof64(i + 1) >= length {
                level = Some(i);
                break;
            }
        }
        let level = level.unwrap();
        let mut sub_entries = Vec::new();
        if level != 0 {
            for _i in 0..(length/Self::powof64(level)) {
                sub_entries.push(RefCell::new(Self::new(Self::powof64(level))))
            }
        }
        
        Self {
            bits: 0,
            level,
            length,
            sub_entries
        }
    }

    fn powof64(p: usize) -> usize {
        match p {
            0  => 1,
            1  => 64,
            2  => 64*64,
            3  => 64*64*64,
            4  => 64*64*64*64,
            5  => 64*64*64*64*64,
            6  => 64*64*64*64*64*64,
            7  => 64*64*64*64*64*64*64,
            8  => 64*64*64*64*64*64*64*64,
            9  => 64*64*64*64*64*64*64*64*64,
            10 => 64*64*64*64*64*64*64*64*64*64,
            _ => panic!("pow64 exceed usize")
        }
    }

    fn subentry_capacity(&self) -> usize {
        Self::powof64(self.level)
    }

    fn set_bit(&mut self, pos: usize) {
        self.bits = self.bits | (1<<pos);
    }

    fn clear_bit(&mut self, pos: usize) {
        self.bits = self.bits & !(1<<pos);
    }

    fn get_bit(&self, pos: usize) -> bool {
        self.bits & (1<<pos) != 0
    }

    pub fn is_full(&self) -> bool {
        self.bits == 0xFFFF_FFFF_FFFF_FFFF
    }
    
    /// return true if all full
    pub fn set(&mut self, pos: usize) -> bool {
        assert!(pos <= self.length, "index out of bound");
        if self.is_full() {
            true
        } else if self.level == 0 {
            self.set_bit(pos);
            self.is_full()
        } else {
            let entry_index = pos / self.subentry_capacity();
            let entry_offset = pos % self.subentry_capacity();
            if self.sub_entries[entry_index].borrow_mut().set(entry_offset) {
                self.set_bit(entry_index);
            }
            self.is_full()
        }
    }
    
    pub fn clear(&mut self, pos: usize) {
        assert!(pos <= self.length, "index out of bound");
        if self.level == 0 {
            self.clear_bit(pos);
        } else {
            let entry_index = pos / self.subentry_capacity();
            let entry_offset = pos % self.subentry_capacity();
            self.sub_entries[entry_index].borrow_mut().clear(entry_offset);
            self.clear_bit(entry_index);
        }
    }

    pub fn set_val(&mut self, pos: usize, val: bool) {
        if val && !self.get(pos) {
            self.set(pos);
        } else if !val && self.get(pos) {
            self.clear(pos);
        }
    }

    pub fn get(&self, pos: usize) -> bool {
        if self.is_full() {
            true
        } else if self.level != 0 {
            let entry_index = pos / self.subentry_capacity();
            let entry_offset = pos % self.subentry_capacity();
            self.sub_entries[entry_index].borrow_mut().get(entry_offset)
        } else {
            self.get_bit(pos)
        }
    }

    pub fn first_empty(&self) -> Option<usize> {
        if self.is_full() {
            None
        } else {
            for i in 0..64 {
                if !self.get_bit(i) {
                    if self.level != 0 {
                        return Some(self.subentry_capacity() * i + self.sub_entries[i].borrow_mut().first_empty().unwrap());
                    } else {
                        return Some(i);
                    }
                }
            }
            unreachable!()
        }
    }
}

pub struct BitMap {
    length: usize,
    start_addr: PhysAddr,
    root_index: BitMapIndex
}

impl BitMap {
    /// total_length in bits
    pub fn new(start_addr: PhysAddr, length: usize) -> Self {
        let mut bi = BitMapIndex::new(length);

        for i in 0..(length/64) {
            bi.set_val(i, unsafe{(start_addr+i).read_volatile::<u64>() == 0xFFFF_FFFF_FFFF_FFFF});
        }
        
        Self {
            length,
            start_addr,
            root_index: bi
        }
    }

    fn raw_get(&self, pos: usize) -> bool {
        if cfg!(debug_assertions) {
            assert!(pos < self.length, "Bitmap oor");
        }
        let arr_index = pos / 64;
        let arr_offset = pos % 64;
        self.raw_get_bits(arr_index) & (1<<arr_offset) != 0
    }

    fn raw_set(&self, pos: usize) {
        if cfg!(debug_assertions) {
            assert!(pos < self.length, "Bitmap oor");
        }
        let arr_index = pos / 64;
        let arr_offset = pos % 64;
        let original_bits = self.raw_get_bits(arr_index);
        unsafe {(self.start_addr + arr_index).write_volatile(&(original_bits | (1 << arr_offset)))}
    }

    fn raw_clear(&self, pos: usize) {
        if cfg!(debug_assertions) {
            assert!(pos < self.length, "Bitmap oor");
        }
        let arr_index = pos / 64;
        let arr_offset = pos % 64;
        let original_bits = self.raw_get_bits(arr_index);
        unsafe {(self.start_addr + arr_index).write_volatile(&(original_bits & !(1 << arr_offset)))}
    }

    /// arr_index for u64
    fn raw_get_bits(&self, arr_index: usize) -> u64 {
        if cfg!(debug_assertions) {
            assert!(arr_index < self.length / 64, "Bitmap oor");
        }
        unsafe {(self.start_addr + arr_index).read_volatile()}
    }

    pub fn get(&self, pos: usize) -> bool {
        if self.root_index.get(pos / 64) {
            true
        } else {
            self.raw_get(pos)
        }
    }

    pub fn set(&mut self, pos: usize) {
        self.raw_set(pos);
        if self.raw_get_bits(pos / 64) == 0xFFFF_FFFF_FFFF_FFFF {
            self.root_index.set(pos / 64);
        }
    }

    pub fn clear(&mut self, pos: usize) {
        self.raw_clear(pos);
        self.root_index.clear(pos / 64);
    }

    pub fn set_val(&mut self, pos: usize, val: bool) {
        if val {
            self.set(pos);
        } else {
            self.clear(pos);
        }
    }

    pub fn first_empty(&self) -> Option<usize> {
        self.root_index.first_empty().and_then(
            |arr_index: usize| -> Option<usize> {
                for i in 0..64 {
                    let pos = arr_index * 64 + i;
                    if !self.raw_get(pos) {
                        return Some(pos);
                    }
                }
                unreachable!()
            }
        )
    }

    pub fn clear_all(&mut self) {
        for i in 0..self.length {
            self.clear(i);
        }
    }

    /// Slow method, only use in profiling!
    pub fn count(&self) -> usize {
        let mut res = 0;
        for i in 0..(self.length/64) {
            res += self.raw_get_bits(i).count_ones() as usize;
        }
        res
    }
}