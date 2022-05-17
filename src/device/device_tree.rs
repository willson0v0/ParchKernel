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

use alloc::borrow::ToOwned;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};
use crate::utils::{ErrorNum, LogLevel, RWLock, SpinRWLock, UUID};
use crate::mem::PhysAddr;
use core::fmt::Debug;
use core::mem::size_of;

use super::device_manager::Driver;


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

#[derive(Clone)]
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
        log!(log_level, " - Reserved memory regions: ");
        if self.reserved_mem.is_empty() {
            log!(log_level, "\t(empty)")
        } else {
            for region in self.reserved_mem.iter() {
                log!(log_level, "\t{:?} ~ {:?} ({} bytes)", region.start, region.start + region.length, region.length);
            }
        }
        log!(log_level, " - Nodes: ");
        for node in self.nodes.iter() {
            node.acquire_r().print(log_level, 1);
        }
    }

    pub fn search_single(&self, field: &str, target: DTBPropertyValue) -> Result<Arc<SpinRWLock<DTBNode>>, ErrorNum> {
        for n in self.nodes.iter() {
            match self.search_inner(field, target.clone(), n.clone())?.as_slice() {
                [] => continue,
                [dev] => return Ok(dev.clone()),
                _ => panic!("Found multiple result"),
            }
        }
        Err(ErrorNum::ENXIO)
    }

    pub fn search(&self, field: &str, target: DTBPropertyValue) -> Result<Vec<Arc<SpinRWLock<DTBNode>>>, ErrorNum> {
        let mut res = Vec::new();
        for n in self.nodes.iter() {
            res.extend(self.search_inner(field, target.clone(), n.clone())?);
        }
        return Ok(res);
    }

    fn search_inner(&self, field: &str, target: DTBPropertyValue, root: Arc<SpinRWLock<DTBNode>>) -> Result<Vec<Arc<SpinRWLock<DTBNode>>>, ErrorNum> {
        let mut res = Vec::new();
        let root_guard = root.acquire_r();
        if let Ok(val) = root_guard.get_value(field) {
            if val.equals(&target)? {
                res.push(root.clone());
            }
        }
        for child in root_guard.children.iter() {
            res.extend(self.search_inner(field, target.clone(), child.clone())?);
        }
        return Ok(res)
    }

    pub fn serach_compatible(&self, compatible: &str) -> Result<Vec<Arc<SpinRWLock<DTBNode>>>, ErrorNum> {
        let mut res = Vec::new();
        for n in self.nodes.iter() {
            res.extend(self.serach_compatible_inner(compatible, n.clone())?);
        }
        return Ok(res);
    }

    fn serach_compatible_inner(&self, compatible: &str, root: Arc<SpinRWLock<DTBNode>>) -> Result<Vec<Arc<SpinRWLock<DTBNode>>>, ErrorNum> {
        let mut res = Vec::new();
        let root_guard = root.acquire_r();
        if let Ok(val) = root_guard.get_value("compatible") {
            if val.contains(compatible)? {
                res.push(root.clone());
            }
        }
        for child in root_guard.children.iter() {
            res.extend(self.serach_compatible_inner(compatible, child.clone())?);
        }
        return Ok(res)
    }

    pub fn hart_count(&self) -> usize {
        self.search("device_type", DTBPropertyValue::CStr("cpu".to_string())).unwrap().len()
    }

    pub fn contains_field(&self, field: &str) -> Result<Vec<Arc<SpinRWLock<DTBNode>>>, ErrorNum> {
        let mut res = Vec::new();
        for n in self.nodes.iter() {
            res.extend(self.contains_field_inner(field, n.clone())?);
        }
        return Ok(res);
    }

    fn contains_field_inner(&self, field: &str, root: Arc<SpinRWLock<DTBNode>>) -> Result<Vec<Arc<SpinRWLock<DTBNode>>>, ErrorNum> {
        let mut res = Vec::new();
        let root_guard = root.acquire_r();
        if root_guard.get_value(field).is_ok() {
            res.push(root.clone());
        }
        for child in root_guard.children.iter() {
            res.extend(self.contains_field_inner(field, child.clone())?);
        }
        return Ok(res)
    }
}

#[derive(Copy, Clone)]
pub struct DTBMemReserve {
    start: PhysAddr,
    length: usize
}

#[derive(Debug, Clone)]
pub enum DTBPropertyValue {
    Empty,
    UInt32(u32),
    UInt64(u64),
    CStr(String),
    CStrList(Vec<String>),
    Custom(Vec<u8>)
}

impl DTBPropertyValue {
    pub fn equals(&self, other: &DTBPropertyValue) -> Result<bool, ErrorNum> {
        match(self, other) {
            (Self::UInt32(l0), Self::UInt32(r0))        => Ok(l0 == r0),
            (Self::UInt64(l0), Self::UInt64(r0))        => Ok(l0 == r0),
            (Self::CStr(l0), Self::CStr(r0))            => Ok(l0 == r0),
            (Self::CStrList(l0), Self::CStrList(r0))    => Ok(l0 == r0),
            (Self::Custom(l0), Self::Custom(r0))        => Ok(l0 == r0),
            _ => Err(ErrorNum::EBADTYPE),
        }
    }

    pub fn contains(&self, tgt: &str) -> Result<bool, ErrorNum> {
        if let Self::CStrList(content) = self {
            Ok(content.iter().any(|x| x == tgt))
        } else {
            Err(ErrorNum::EBADTYPE)
        }
    }
}

impl DTBPropertyValue {
    pub fn from_bytes(name: String, value: Vec<u8>) -> Result<Self, ErrorNum> {
        let res = match name.as_str() {
            "riscv,isa"             => Self::CStr(Self::read_cstr(value)?),
            "mmu-type"              => Self::CStr(Self::read_cstr(value)?),
            "compatible"            => Self::CStrList(Self::read_cstr_list(value)?),
            "model"                 => Self::CStr(Self::read_cstr(value)?),
            "device_type"           => Self::CStr(Self::read_cstr(value)?),
            "phandle"               => Self::UInt32(Self::read_u32(value)?),
            "status"                => Self::CStr(Self::read_cstr(value)?),
            "#address-cells"        => Self::UInt32(Self::read_u32(value)?),
            "#size-cells"           => Self::UInt32(Self::read_u32(value)?),
            "reg"                   => Self::Custom(value),
            "virtual-reg"           => Self::UInt32(Self::read_u32(value)?),
            "ranges"                => Self::Custom(value),
            "dma-range"             => Self::Custom(value),
            "interrupts"            => Self::UInt32(Self::read_u32(value)?),
            "interrupt-parent"      => Self::UInt32(Self::read_u32(value)?),
            "#interrupt-cells"      => Self::UInt32(Self::read_u32(value)?),
            "interrupt-controller"  => Self::Empty,
            "interrupts-extended"   => Self::Custom(value),
            "interrupt-map-mask"    => Self::Custom(value),
            "regmap"                => Self::UInt32(Self::read_u32(value)?),
            "offset"                => Self::UInt32(Self::read_u32(value)?),
            "value"                 => Self::UInt32(Self::read_u32(value)?),
            "cpu"                   => Self::UInt32(Self::read_u32(value)?),
            "clock-frequency"       => {
                if value.len() == size_of::<u32>() {
                    Self::UInt32(Self::read_u32(value)?)
                } else if value.len() == size_of::<u64>() {
                    Self::UInt64(Self::read_u64(value)?)
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

    fn read_cstr_list(value: Vec<u8>) -> Result<Vec<String>, ErrorNum> {
        value.split(|byte| *byte == 0).filter(|arr| arr.len() != 0).map(|arr| Self::read_cstr(arr.to_vec())).collect::<Result<Vec<String>, ErrorNum>>()
    }

    fn read_cstr(value: Vec<u8>) -> Result<String, ErrorNum> {
        let mut res = String::from_utf8(value).map_err(|_| ErrorNum::EBADCODEX)?;
        if res.ends_with("\0") {
            res.pop();
        }
        Ok(res)
    }

    fn read_u32(value: Vec<u8>) -> Result<u32, ErrorNum> {
        Ok(u32::from_be_bytes(value.try_into().map_err(|_| ErrorNum::EBADDTB)?))
    }

    fn read_u64(value: Vec<u8>) -> Result<u64, ErrorNum> {
        Ok(u64::from_be_bytes(value.try_into().map_err(|_| ErrorNum::EBADDTB)?))
    }

    pub fn get_u32(&self) -> Result<u32, ErrorNum> {
        match self {
            DTBPropertyValue::UInt32(val) => Ok(*val),
            _ => Err(ErrorNum::EBADTYPE)
        }
    }

    pub fn get_u64(&self) -> Result<u64, ErrorNum> {
        match self {
            DTBPropertyValue::UInt64(val) => Ok(*val),
            _ => Err(ErrorNum::EBADTYPE)
        }
    }

    pub fn get_cstr(&self) -> Result<String, ErrorNum> {
        match self {
            DTBPropertyValue::CStr(val) => Ok(val.to_owned()),
            _ => Err(ErrorNum::EBADTYPE)
        }
    }


    pub fn get_custom(&self) -> Result<Vec<u8>, ErrorNum> {
        match self {
            DTBPropertyValue::Custom(val) => Ok(val.to_owned()),
            _ => Err(ErrorNum::EBADTYPE)
        }
    }
}


pub struct AddressSizePair{
    pub address: usize,
    pub size: usize
}

pub struct DTBNode {
    pub unit_name: String,
    pub properties: Vec<(String, DTBPropertyValue)>,
    pub children: Vec<Arc<SpinRWLock<DTBNode>>>,
    pub parent: Option<Weak<SpinRWLock<DTBNode>>>,
    pub driver: UUID
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

    pub fn is_compatible(&self, compatible: &str) -> bool {
        for (name, value) in self.properties.iter() {
            if name.as_str() == "compatible" {
                if let DTBPropertyValue::CStrList(comp) = value {
                    for c in comp {
                        if c.as_str() == compatible {
                            return true;
                        }
                    }
                } else {
                    panic!("bad property value type");
                }
            }
        }
        false
    }

    pub fn get_value(&self, key: &str) -> Result<DTBPropertyValue, ErrorNum> {
        for (name, value) in self.properties.iter() {
            if name == key {
                return Ok(value.to_owned());
            }
        }
        Err(ErrorNum::EBADDTB)
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
            parent,
            driver: UUID::new()
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

    pub fn reg_value(&self) -> Result<Vec<AddressSizePair>, ErrorNum> {
        let mut res = Vec::new();

        let address_cells = if let Some(parent) = self.parent.clone() {
            parent.upgrade().unwrap().acquire_r().get_value("#address-cells").unwrap_or({
                warning!("Parent node doesn't have property #address-cells, using default (2)");
                DTBPropertyValue::UInt32(2)
            }).get_u32().unwrap() as usize
        } else {
            warning!("Parent node doesn't exist, #address-cells using default (2)");
            2
        };

        let size_cells = if let Some(parent) = self.parent.clone() {
            parent.upgrade().unwrap().acquire_r().get_value("#size-cells").unwrap_or({
                warning!("Parent node doesn't have property #size-cells, using default (1)");
                DTBPropertyValue::UInt32(1)
            }).get_u32().unwrap() as usize
        } else {
            warning!("Parent node doesn't exist, #size-cells using default (2)");
            1
        };

        let byte_arr = self.get_value("reg")?.get_custom()?;
        let mut ptr = 0usize;
        let address_bytes = address_cells * size_of::<u32>();
        let size_bytes = size_cells * size_of::<u32>();
        if byte_arr.len() % (address_bytes + size_bytes) != 0 {
            warning!("bad reg length");
            return Err(ErrorNum::EBADDTB);
        }
        while ptr < byte_arr.len() {
            let address = Self::be_bytes_to_u64(&byte_arr[ptr..ptr + address_bytes]);
            ptr += address_bytes;
            let size = Self::be_bytes_to_u64(&byte_arr[ptr..ptr + size_bytes]);
            ptr += size_bytes;
            res.push(AddressSizePair{
                address,
                size
            });
        }

        Ok(res)
    }

    fn be_bytes_to_u64(slice: &[u8]) -> usize {
        debug_assert!(slice.len() <= 8);
        let mut buffer = [0u8; 8];
        buffer[16-slice.len()..].copy_from_slice(slice);
        usize::from_be_bytes(buffer)
    }
}