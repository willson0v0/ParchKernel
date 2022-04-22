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

use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};
use crate::utils::{ErrorNum, LogLevel, RWLock, SpinRWLock};
use crate::mem::PhysAddr;
use core::fmt::Debug;
use core::mem::size_of;


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
    pub address: u64,
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
        self.address == 0 && self.size == 0
    }
}

crate::enum_with_tryfrom_u32! {
    /// Type of FDTTokens
    #[repr(u32)]
    enum FDTTokenType {
        /// Marks the begining of a node's representation.
        BeginNode   = u32::from_be(0x00000001),
        /// Marks the end of a node's representation.
        EndNode     = u32::from_be(0x00000002),
        /// Property in a node's representation.
        Property    = u32::from_be(0x00000003),
        /// Ignored
        Nop         = u32::from_be(0x00000004),
        /// End of the whole structure block
        End         = u32::from_be(0x00000009),
    }
}

#[derive(Debug)]
struct FDTBeginNodeToken {
    unit_name: String
}

#[derive(Debug)]
struct FDTPropertyToken {
    length: u32,
    offset: u32,
    value: Vec<u8>
}

#[derive(Debug)]
enum FDTToken {
    BeginNode(FDTBeginNodeToken),
    EndNode,
    Property(FDTPropertyToken),
    Nop,
    End
}

impl FDTToken {
    /// read_volatile will copy data so ownership should be fine
    pub fn read_token(addr: PhysAddr) -> Result<(FDTToken, PhysAddr), ErrorNum> {
        let token_type = FDTTokenType::try_from(unsafe { addr.read_volatile::<u32>() })?;
        let nxt_ptr = addr + core::mem::size_of::<FDTTokenType>();
        match token_type {
            FDTTokenType::BeginNode => {
                let unit_name = nxt_ptr.read_cstr();
                let mut len = unit_name.len();
                if len % 4 != 0 {
                    len += 4 - (len % 4);
                }
                loop {
                    let nxt: u32 = unsafe{(nxt_ptr + len).read_volatile()};
                    if nxt == 0 {
                        len += 4;
                    } else {
                        break;
                    }
                }
                Ok((FDTToken::BeginNode(FDTBeginNodeToken{unit_name}), nxt_ptr + len))
            },
            FDTTokenType::EndNode => Ok((FDTToken::EndNode, nxt_ptr)),
            FDTTokenType::Property => {
                let length = u32::from_be(unsafe{nxt_ptr.read_volatile()});
                let offset = u32::from_be(unsafe{(nxt_ptr + 4).read_volatile()});
                let value = (nxt_ptr + 8).read_str(length as usize);
                let mut len = 8usize + length as usize;
                if len % 4 != 0 {
                    len += 4 - (len % 4);
                }
                Ok((FDTToken::Property(FDTPropertyToken{ length, offset, value }), nxt_ptr + len))
            },
            FDTTokenType::Nop => Ok((FDTToken::Nop, nxt_ptr)),
            FDTTokenType::End => Ok((FDTToken::End, nxt_ptr)),
        }
    }
}

pub struct DeviceTree {
    reserved_mem: Vec<DTBMemReserve>,
    nodes: Vec<Arc<SpinRWLock<DTBNode>>>,
}

impl DeviceTree {
    pub fn parse(addr: PhysAddr) -> Result<Self, ErrorNum> {
        verbose!("Parsing on {:?}", addr);
        let header: FDTHeader = unsafe { addr.read_volatile() };
        if header.magic != 0xD00DFEED_u32.to_be() {
            warning!("Bad dtb magic number");
            return Err(ErrorNum::EBADDTB)
        }

        let rsvmap_addr = addr + u32::from_be(header.rsvmap_offset) as usize;
        let struct_addr = addr + u32::from_be(header.struct_offset) as usize;
        let string_addr = addr + u32::from_be(header.string_offset) as usize;

        verbose!("rsvmap_addr: {:?}", rsvmap_addr);
        verbose!("struct_addr: {:?}", struct_addr);
        verbose!("string_addr: {:?}", string_addr);

        let reserved_mem = FDTReserveEntry::get_entries(rsvmap_addr)?.into_iter().map(|fdt_entry| DTBMemReserve {
            start: (u64::from_be(fdt_entry.address) as usize).into(),
            length: u64::from_be(fdt_entry.size) as usize,
        }).collect();

        let mut nodes = Vec::new();
        let mut iter = struct_addr;
        loop {
            let res = DTBNode::read_node(iter, string_addr, None)?;
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
            node.acquire_r().print(log_level, 0);
        }
    }
}

pub struct DTBMemReserve {
    start: PhysAddr,
    length: usize
}

#[derive(Debug)]
pub enum DTBPropertyValue {
    Empty,
    UInt32(u32),
    UInt64(u64),
    CStr(String),
    CStrList(Vec<String>),
    Custom(Vec<u8>)
}

impl DTBPropertyValue {
    pub fn from_bytes(name: String, value: Vec<u8>) -> Result<Self, ErrorNum> {
        let res = match name.as_str() {
            "riscv,isa"             => Self::CStr(Self::get_cstr(value)?),
            "mmu-type"              => Self::CStr(Self::get_cstr(value)?),
            "compatible"            => Self::CStrList(Self::get_cstr_list(value)?),
            "model"                 => Self::CStr(Self::get_cstr(value)?),
            "device_type"           => Self::CStr(Self::get_cstr(value)?),
            "phandle"               => Self::UInt32(Self::get_u32(value)?),
            "status"                => Self::CStr(Self::get_cstr(value)?),
            "#address-cells"        => Self::UInt32(Self::get_u32(value)?),
            "#size-cells"           => Self::UInt32(Self::get_u32(value)?),
            "reg"                   => Self::Custom(value),
            "virtual-reg"           => Self::UInt32(Self::get_u32(value)?),
            "ranges"                => Self::Custom(value),
            "dma-range"             => Self::Custom(value),
            "interrupts"            => Self::UInt32(Self::get_u32(value)?),
            "interrupt-parent"      => Self::UInt32(Self::get_u32(value)?),
            "#interrupt-cells"      => Self::UInt32(Self::get_u32(value)?),
            "interrupt-controller"  => Self::Empty,
            "interrupts-extended"   => Self::Custom(value),
            "interrupt-map-mask"    => Self::Custom(value),
            "regmap"                => Self::UInt32(Self::get_u32(value)?),
            "offset"                => Self::UInt32(Self::get_u32(value)?),
            "value"                 => Self::UInt32(Self::get_u32(value)?),
            "cpu"                   => Self::UInt32(Self::get_u32(value)?),
            "clock-frequency"       => {
                if value.len() == size_of::<u32>() {
                    Self::UInt32(Self::get_u32(value)?)
                } else if value.len() == size_of::<u64>() {
                    Self::UInt64(Self::get_u64(value)?)
                } else {
                    return Err(ErrorNum::EBADDTB)
                }
            },
            unknown => {
                warning!("Unrecognized property {} in DTB", unknown);
                Self::Custom(value)
            }
        };
        Ok(res)
    }

    fn get_cstr_list(value: Vec<u8>) -> Result<Vec<String>, ErrorNum> {
        value.split(|byte| *byte == 0).filter(|arr| arr.len() != 0).map(|arr| Self::get_cstr(arr.to_vec())).collect::<Result<Vec<String>, ErrorNum>>()
    }

    fn get_cstr(value: Vec<u8>) -> Result<String, ErrorNum> {
        let mut res = String::from_utf8(value).map_err(|_| ErrorNum::EBADCODEX)?;
        if res.ends_with("\0") {
            res.pop();
        }
        Ok(res)
    }

    fn get_u32(value: Vec<u8>) -> Result<u32, ErrorNum> {
        Ok(u32::from_be_bytes(value.try_into().map_err(|_| ErrorNum::EBADDTB)?))
    }

    fn get_u64(value: Vec<u8>) -> Result<u64, ErrorNum> {
        Ok(u64::from_be_bytes(value.try_into().map_err(|_| ErrorNum::EBADDTB)?))
    }
}

pub struct DTBNode {
    unit_name: String,
    properties: Vec<(String, DTBPropertyValue)>,
    children: Vec<Arc<SpinRWLock<DTBNode>>>,
    parent: Option<Weak<SpinRWLock<DTBNode>>>,
}

impl DTBNode {
    pub fn print(&self, log_level: LogLevel, indent: usize) {
        let indent_str: String = (0..indent).map(|_| "\t").collect();
        log!(log_level, "{}Node <{}>", indent_str, self.unit_name);
        for property in self.properties.iter() {
            log!(log_level, "{} - {}: {:?}", indent_str, property.0, property.1);
        }
        if !self.children.is_empty() {
            log!(log_level, "{} - children:", indent_str);
            for child in self.children.iter() {
                child.acquire_r().print(log_level, indent+1);
            }
        }
    }

    /// return node & it's end position's next address
    pub fn read_node(start: PhysAddr, str_block: PhysAddr, parent: Option<Weak<SpinRWLock<DTBNode>>>) -> Result<Option<(Arc<SpinRWLock<DTBNode>>, PhysAddr)>, ErrorNum> {
        verbose!("Parsing node from {:?}", start);
        #[derive(Debug)]
        enum FSMState {
            Begin,
            Property,
            Child,
        }

        let mut state = FSMState::Begin;
        let mut iter = start;
        let node = Arc::new(SpinRWLock::new(DTBNode {
            unit_name: "".into(),
            properties: Vec::new(),
            children: Vec::new(),
            parent
        }));
        let node_clone = node.clone();
        let mut node_guard = node_clone.acquire_w();
        loop {
            let (token, nxt_addr) = FDTToken::read_token(iter)?;
            verbose!("reading on {:?}, current token {:?}, current state {:?}", iter, token, state);
            match state {
                FSMState::Begin => {
                    match token {
                        FDTToken::BeginNode(token) => {
                            node_guard.unit_name = token.unit_name;
                            state = FSMState::Property;
                            iter = nxt_addr;
                        },
                        FDTToken::Nop => {
                            iter = nxt_addr;
                        },
                        FDTToken::EndNode => {
                            warning!("token {:?} found when in {:?} state", token, state);
                            return Err(ErrorNum::EBADDTB)
                        },
                        _ => {
                            return Ok(None)
                        }
                    }
                },
                FSMState::Property => {
                    match token {
                        FDTToken::Property(token) => {
                            iter = nxt_addr;
                            let name = (str_block + token.offset as usize).read_cstr();
                            node_guard.properties.push((name.clone(), DTBPropertyValue::from_bytes(name, token.value)?));
                        },
                        FDTToken::Nop => {
                            iter = nxt_addr;
                        },
                        FDTToken::BeginNode(_) => {
                            state = FSMState::Child;
                        },
                        FDTToken::EndNode => return Ok(Some((node, nxt_addr))),
                        _ => {
                            warning!("token {:?} found when in {:?} state", token, state);
                            return Err(ErrorNum::EBADDTB)
                        },
                    }
                },
                FSMState::Child => {
                    match token {
                        FDTToken::BeginNode(_) => {
                            let child_res = Self::read_node(iter, str_block, Some(Arc::downgrade(&node)))?;
                            if let Some((child, addr)) = child_res {
                                iter = addr;
                                node_guard.children.push(child);
                            } else {
                                // starts with BeginNode, must have child, not format error, panic
                                panic!("dtb no child?")
                            }
                        },
                        FDTToken::EndNode => return Ok(Some((node, nxt_addr))),
                        _ => {
                            warning!("token {:?} found when in {:?} state", token, state);
                            return Err(ErrorNum::EBADDTB)
                        },
                    }
                },
            }
        }
    }
}