//! Device Tree Parser.
//! 
//! DTB Format
//! 
//! ===== head =====
//! struct FDTHeader
//! ----------------
//! free space
//! ----------------
//! memory reservation block
//! ----------------
//! free space
//! ----------------
//! structure block
//! ----------------
//! free space
//! ----------------
//! string block
//! ----------------
//! free space
//! ===== tail =====

use alloc::{string::String, vec::Vec};
use crate::utils::{ErrorNum, LogLevel};
use crate::mem::PhysAddr;
use core::fmt::Debug;


/// The header of .dtb file (Flattened Devicetree)
#[repr(C)]
struct FDTHeader {
    /// Big-endian, 0xD00DFEED
    pub magic           : u32,  
    /// total size in bytes of the device tree data structure. 
    pub total_size      : u32,  
    /// offset in bytes of the structure block from the beginning of the header 
    pub struct_offset   : u32,  
    /// offset in bytes of the memory reservation block from the beginning of the header 
    pub string_offset   : u32,
    /// offset in bytes of the memory reservation block from the beginning of the header 
    pub rsvmap_offset   : u32,
    /// dts version
    pub version         : u32,
    /// lowest backward compatable version
    pub last_comp_ver   : u32,
    /// size of string block section
    pub string_size     : u32,
    /// size of structure block section
    pub struct_size     : u32
}

/// The memory reservation block consistes of a list of entry of this format.
/// Each block specified here should not be accessed.
/// The list end with a entry with all zero
#[repr(C)]
struct FDTReserveEntry {
    pub address: PhysAddr,
    pub size: u64
}

impl FDTReserveEntry {
    pub fn get_entries(addr: PhysAddr) -> Result<Vec<FDTReserveEntry>, ErrorNum> {
        let mut res = Vec::new();
        let mut iter = addr;
        loop {
            let entry: Self = unsafe {iter.read_volatile()};
            if entry.is_end() {
                return Ok(res);
            }
            res.push(entry);
            iter += core::mem::size_of::<Self>();
        }
    }

    pub fn is_end(&self) -> bool {
        self.address.0 == 0 && self.size == 0
    }
}

/// Type of FDTTokens
#[repr(u32)]
enum FDTTokenType {
    /// Marks the begining of a node's representation.
    BeginNode   = 0x00000001,
    /// Marks the end of a node's representation.
    EndNode     = 0x00000002,
    /// Property in a node's representation.
    Property    = 0x00000003,
    /// Ignored
    Nop         = 0x00000004,
    /// End of the whole structure block
    End         = 0x00000005,
}

struct FDTBeginNodeToken {
    unit_name: String
}

struct FDTPropertyToken {
    length: u32,
    offset: u32
}

enum FDTToken {
    BeginNode(FDTBeginNodeToken),
    EndNode,
    Property(FDTPropertyToken),
    Nop,
    End
}

impl FDTToken {
    /// read_volatile will copy data so ownership should be fine
    pub fn read_token(addr: PhysAddr) -> (FDTToken, PhysAddr) {
        let token_type: FDTTokenType = unsafe { addr.read_volatile() };
        let nxt_ptr = addr + core::mem::size_of::<FDTTokenType>();
        match token_type {
            FDTTokenType::BeginNode => {
                let unit_name = nxt_ptr.read_cstr();
                let len = unit_name.len();
                (FDTToken::BeginNode(FDTBeginNodeToken{unit_name}), nxt_ptr + len)
            },
            FDTTokenType::EndNode => (FDTToken::EndNode, nxt_ptr),
            FDTTokenType::Property => {
                (FDTToken::Property(unsafe{nxt_ptr.read_volatile()}), nxt_ptr + core::mem::size_of::<FDTPropertyToken>())
            },
            FDTTokenType::Nop => (FDTToken::Nop, nxt_ptr),
            FDTTokenType::End => (FDTToken::End, nxt_ptr),
        }
    }
}

pub struct DeviceTree {
    reserved_mem: Vec<DTBMemReserve>,
    nodes: Vec<DTBNode>,
}

impl DeviceTree {
    pub fn parse(addr: PhysAddr) -> Result<Self, ErrorNum> {
        let header: FDTHeader = unsafe { addr.read_volatile() };
        if header.magic != 0xD00DFEED {
            return Err(ErrorNum::EBADDTB)
        }

        let rsvmap_addr = addr + header.rsvmap_offset as usize;
        let struct_addr = addr + header.struct_offset as usize;
        let string_addr = addr + header.string_offset as usize;

        let reserved_mem = FDTReserveEntry::get_entries(rsvmap_addr)?.into_iter().map(|fdt_entry| DTBMemReserve {
            start: fdt_entry.address,
            length: fdt_entry.size as usize,
        }).collect();

        let mut nodes = Vec::new();
        let mut iter = struct_addr;
        loop {
            let res = DTBNode::read_node(iter, string_addr)?;
            if let Some((node, nxt_start)) = res {
                nodes.push(node);
                iter = nxt_start;
            } else {
                break;
            }
        }

        Ok(Self {
            reserved_mem,
            nodes,
        })
    }

    pub fn print(&self, log_level: LogLevel) {
        log!(log_level, "===== DeviceTree print begin =====");
        log!(log_level, "- Reserved memory regions: ");
        for region in self.reserved_mem.iter() {
            log!(log_level, "\t{:?} ~ {:?} ({} bytes)", region.start, region.start + region.length, region.length);
        }
        log!(log_level, "- Nodes: ");
        for node in self.nodes.iter() {
            node.print(log_level, 0);
        }
    }
}

pub struct DTBMemReserve {
    start: PhysAddr,
    length: usize
}

pub struct DTBNode {
    unit_name: String,
    properties: Vec<String>,
    children: Vec<DTBNode>,
}

impl DTBNode {
    pub fn print(&self, log_level: LogLevel, indent: usize) {
        let indent_str: String = (0..indent).map(|_| "\t").collect();
        log!(log_level, "{}{}", indent_str, self.unit_name);
        log!(log_level, "{}- properties:", indent_str);
        for property in self.properties.iter() {
            log!(log_level, "{}\t- {}", indent_str, property);
        }
        log!(log_level, "{}- children:", indent_str);
        for child in self.children.iter() {
            child.print(log_level, indent+1);
            log!(log_level, "");
        }
    }

    /// return node & it's end position's next address
    pub fn read_node(start: PhysAddr, str_block: PhysAddr) -> Result<Option<(DTBNode, PhysAddr)>, ErrorNum> {
        enum FSMState {
            Begin,
            Property,
            Child,
        }

        let mut state = FSMState::Begin;
        let mut iter = start;
        let mut node: DTBNode = DTBNode {
            unit_name: "".into(),
            properties: Vec::new(),
            children: Vec::new(),
        };
        loop {
            let (token, nxt_addr) = FDTToken::read_token(iter);
            match state {
                FSMState::Begin => {
                    match token {
                        FDTToken::BeginNode(token) => {
                            node.unit_name = token.unit_name;
                            state = FSMState::Property;
                            iter = nxt_addr;
                        },
                        FDTToken::Nop => {
                            iter = nxt_addr;
                        },
                        FDTToken::EndNode => {
                            return Ok(None);
                        },
                        _ => return Err(ErrorNum::EBADDTB)
                    }
                },
                FSMState::Property => {
                    match token {
                        FDTToken::Property(token) => {
                            iter = nxt_addr;
                            node.properties.push((str_block + token.offset as usize).read_str(token.length as usize));
                        },
                        FDTToken::Nop => {
                            iter = nxt_addr;
                        },
                        FDTToken::BeginNode(_) => {
                            state = FSMState::Child;
                        },
                        FDTToken::EndNode => return Ok(Some((node, nxt_addr))),
                        _ => return Err(ErrorNum::EBADDTB),
                    }
                },
                FSMState::Child => {
                    match token {
                        FDTToken::BeginNode(_) => {
                            let child_res = Self::read_node(iter, str_block)?;
                            if let Some((child, addr)) = child_res {
                                iter = addr;
                                node.children.push(child);
                            } else {
                                // starts with BeginNode, must have child, not format error, panic
                                panic!("dtb no child?")
                            }
                        },
                        FDTToken::EndNode => return Ok(Some((node, nxt_addr))),
                        _ => return Err(ErrorNum::EBADDTB),
                    }
                },
            }
        }
    }
}

fn test(a: FDTHeader) {
    let b = a.string_offset;
}
