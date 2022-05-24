//! UART driver for /dev/pts
//! kernel print use utils/uart.rs

use core::mem::size_of;
use alloc::{boxed::Box, collections::VecDeque, string::ToString, sync::Arc, vec::Vec};

use crate::{device::{device_manager::Driver, device_tree::DTBPropertyValue}, mem::PhysAddr, process::get_processor, utils::{Mutex, MutexGuard, RWLock, SpinMutex, UUID, cast_bytes}};
use core::{any::Any, fmt::Debug};
use crate::utils::ErrorNum;
use bitflags::*;

pub struct UART {
    base_address: PhysAddr,
    clock_freq: u32,
    operator: SpinMutex<UARTOperator>,
    buffer_r: SpinMutex<VecDeque<u8>>,
    buffer_w: SpinMutex<VecDeque<u8>>,
}

struct UARTOperator{
    base_address: PhysAddr,
    rcvr_length: RCVRLength,
}

enum_with_tryfrom_usize!{
    #[repr(usize)]
    pub enum IOCtlOp {
        WriteByte = 1,
        ReadByte = 2,
        Config = 3,
        Sync = 4,
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ParityMode {
    EvenParity,
    OddParity
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StickyParity {
    Enable,
    Disable
}

#[derive(Copy, Clone, Debug)]
pub enum Parity {
    Enable(ParityMode, StickyParity),
    Disable
}

#[derive(Copy, Clone, Debug)]
pub enum StopBit {
    One,
    OneAndHalf,
    Two
}

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBit,
    pub rcvr_length: RCVRLength,
}


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RCVRLength {
    One,
    Four,
    Eight,
    Fourteen
}

#[derive(Copy, Clone, Debug)]
pub enum IOCtlParam {
    Write(u8),
    Read,
    Config(Config),
    Sync,
}

pub enum IOCtlRes {
    Write,
    Read(u8),
    Config,
    Sync,
}

#[repr(u8)]
#[derive(Debug)]
pub enum IntStatus {
    ModemStatus     = 0b00000000,
    THREmpty        = 0b00001000,
    RecvAvail       = 0b00010000,
    RecvLineStatus  = 0b00011000,
    TimeOut         = 0b00110000,
}


bitflags! {
    pub struct LSRFlags: u8 {
        const RECV_DATA_READY   = 0b00000001;
        const OVERRUN_ERROR     = 0b00000010;
        const PARITY_ERROR      = 0b00000100;
        const FRAMING_ERROR     = 0b00001000;
        const BREAK_INTERRUPT   = 0b00010000;
        const FIFO_AVAILABLE    = 0b00100000;
        const FIFO_EMPTT        = 0b01000000;
        const FIFO_ERROR        = 0b10000000;
    }
}

bitflags! {
    pub struct IntFlag: u8 {
        const RECV_READY        = 0b00000001;
        const TRANSMITTER_EMPTY = 0b00000010;
        const RECV_STATUS       = 0b00000100;
        const MODEL_STATUS      = 0b00001000;
    }
}

impl core::convert::TryFrom<u8> for IntStatus {
    type Error = ErrorNum;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == (IntStatus::ModemStatus       as u8) & 0b00111000 => Ok(IntStatus::ModemStatus      ),
            x if x == (IntStatus::THREmpty          as u8) & 0b00111000 => Ok(IntStatus::THREmpty         ),
            x if x == (IntStatus::RecvAvail         as u8) & 0b00111000 => Ok(IntStatus::RecvAvail        ),
            x if x == (IntStatus::RecvLineStatus    as u8) & 0b00111000 => Ok(IntStatus::RecvLineStatus   ),
            x if x == (IntStatus::TimeOut           as u8) & 0b00111000 => Ok(IntStatus::TimeOut          ),
            _ => Err(ErrorNum::EINVAL),
        }
    }
}

impl UARTOperator {
    fn transmitter_holding_buffer          (&self) -> PhysAddr { self.base_address + 0x0 }
    fn receiver_buffer                     (&self) -> PhysAddr { self.base_address + 0x0 }
    fn divisor_latch_low_byte              (&self) -> PhysAddr { self.base_address + 0x0 }
    fn interrupt_enable_register           (&self) -> PhysAddr { self.base_address + 0x1 }
    fn divisor_latch_high_byte             (&self) -> PhysAddr { self.base_address + 0x1 }
    fn interrupt_identification_register   (&self) -> PhysAddr { self.base_address + 0x2 }
    fn fifo_control_register               (&self) -> PhysAddr { self.base_address + 0x2 }
    fn line_control_register               (&self) -> PhysAddr { self.base_address + 0x3 }
    fn modem_control_register              (&self) -> PhysAddr { self.base_address + 0x4 }
    fn line_status_register                (&self) -> PhysAddr { self.base_address + 0x5 }
    fn modem_status_register               (&self) -> PhysAddr { self.base_address + 0x6 }
    fn scratch_register                    (&self) -> PhysAddr { self.base_address + 0x7 }

    fn read_reg(&self, reg: PhysAddr) -> u8 {
        unsafe {reg.read_volatile()}
    }

    fn write_reg(&self, reg: PhysAddr, value: u8) {
        unsafe {reg.write_volatile(&value)}
    }

    pub fn write(&self, b: u8) -> Result<(), ErrorNum> {
        if self.read_reg(self.line_status_register()) & 0b001000000 == 0 {
            return Err(ErrorNum::EAGAIN);
        } else {
            self.write_reg(self.transmitter_holding_buffer(), b);
            return Ok(())
        }
    }

    pub fn read(&self) -> Result<u8, ErrorNum> {
        if self.read_reg(self.line_status_register()) & 0b00000001 == 0 {
            return Err(ErrorNum::EAGAIN);
        } else {
            let res = self.read_reg(self.receiver_buffer());
            return Ok(res)
        }
    }

    pub fn config(&mut self, clock_freq: u32, param: Config) -> Result<(), ErrorNum> {
        // check divisor
        if clock_freq % (16 * param.baud_rate) != 0 {
            return Err(ErrorNum::EINVAL)
        }

        // check rcvr_length
        self.rcvr_length = param.rcvr_length;

        let divisor = clock_freq / (16 * param.baud_rate);
        // bit 7 enable divisor latch access
        self.write_reg(self.line_control_register(), 0b10000000);
        // divisor latch lower
        self.write_reg(self.divisor_latch_low_byte(), (divisor & 0xFF) as u8);
        // divisor latch higher
        self.write_reg(self.divisor_latch_high_byte(), ((divisor >> 8) & 0xFF) as u8);
        // disable divisor latch, 8 bit, no parity, 1 stop bit
        let data_bits = match param.data_bits {
            DataBits::Five => 0b00u8,
            DataBits::Six => 0b01u8,
            DataBits::Seven => 0b10u8,
            DataBits::Eight => 0b11u8,
        };
        let stop_bits = match param.stop_bits {
            StopBit::One => 0b000u8,
            StopBit::OneAndHalf => {
                if data_bits != 0b00 {
                    return Err(ErrorNum::EINVAL);
                }
                0b100u8
            },
            StopBit::Two => {
                if data_bits == 0b00 {
                    return Err(ErrorNum::EINVAL);
                }
                0b100u8
            },
        };
        let parity_bits = match param.parity {
            Parity::Enable(mode, sticky) => {
                let mut res = 0b1000u8; // enable
                res |= match mode {
                    ParityMode::EvenParity => 0b10000u8,
                    ParityMode::OddParity => 0b00000u8,
                };
                if sticky == StickyParity::Enable {
                    res |= 0b100000;
                }
                res
            },
            Parity::Disable => 0b000000,
        };
        // Do we need to break here?
        self.write_reg(self.line_control_register(), data_bits | stop_bits | parity_bits);
        // reset and enable fifo
        self.write_reg(self.fifo_control_register(), 0b00000111);
        // enable rx/tx interrupt
        self.write_reg(self.interrupt_enable_register(), (IntFlag::RECV_READY | IntFlag::TRANSMITTER_EMPTY).bits());

        Ok(())
    }

    pub fn read_int_cause(&self) -> Result<IntStatus, ErrorNum> {
        // impossible to fail, panic on mismatch
        Ok(IntStatus::try_from(self.read_reg(self.interrupt_identification_register())).unwrap())
    }
    
    pub fn deplete_r_buffer(&self, r_buffer: &mut VecDeque<u8>) {
        while LSRFlags::from_bits(self.read_reg(self.line_status_register())).unwrap().contains(LSRFlags::RECV_DATA_READY) {
            r_buffer.push_back(self.read().unwrap());
        }
    }

    pub fn dump_w_buffer(&self, w_buffer: &mut MutexGuard<VecDeque<u8>>) {
        loop {
            if w_buffer.is_empty() {
                return;
            }
            let flags = LSRFlags::from_bits(self.read_reg(self.line_status_register())).unwrap();
            if flags.contains(LSRFlags::FIFO_AVAILABLE) {
                self.write(w_buffer.pop_front().unwrap()).unwrap();
            }
        }
    }
}

impl Debug for UART {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UART @ {:?}", self.base_address)
    }
}

impl UART {
    fn write_byte(&self, b: u8) {
        self.buffer_w.acquire().push_back(b);
    }

    fn write_arr(&self, arr: Vec<u8>) {
        // operator first, buffer next
        let operator = self.operator.acquire();
        let mut buffer_w = self.buffer_w.acquire();
        buffer_w.extend(arr);
        operator.dump_w_buffer(&mut buffer_w);
    }

    fn read_byte(&self) -> u8 { 
        let operator = self.operator.acquire();
        // check buffer
        let mut buffer_r = self.buffer_r.acquire();
        if !buffer_r.is_empty() {
            return buffer_r.pop_front().unwrap();
        }
        drop(buffer_r);
        // check fifo, hold operator in case kernel need read
        if let Ok(b) = operator.read() {
            return b;
        }
        drop(operator);
        loop {
            let core = get_processor();
            if core.current().is_some() {
                // sleep if is user program
                core.suspend_switch();
            }
            if let Ok(b) = self.operator.acquire().read() {
                return b;
            }
        }
    }
}

impl Driver for UART {
    fn new(dev_tree: crate::device::DeviceTree) -> Result<alloc::vec::Vec<(UUID, alloc::sync::Arc<dyn Driver>)>, crate::utils::ErrorNum> where Self: Sized {
        let mut res = Vec::new();
        
        let mut compatible = dev_tree.serach_compatible("ns16550a")?;
        compatible.extend(dev_tree.serach_compatible("ns8250")?);
        for c in compatible {
            let node = c.acquire_r();
            let uuid = node.driver;
            verbose!("Creating Driver instance for {} with uuid {}.", node.unit_name, uuid);
            let base_address: PhysAddr = node.reg_value()?[0].address.into();
            let clock_freq = node.get_value("clock-frequency")?.get_u32()?;
            let driver = Self {
                base_address,
                clock_freq,
                operator: SpinMutex::new("UART", UARTOperator{
                    base_address,
                    rcvr_length: RCVRLength::One, // FIFO buffer default to 1
                }),
                buffer_r: SpinMutex::new("UART", VecDeque::new()),
                buffer_w: SpinMutex::new("UART", VecDeque::new()),
            };
            res.push((uuid, Arc::new(driver).as_driver()));
        }

        Ok(res)
    }

    fn initialize(&self) -> Result<(), crate::utils::ErrorNum> {
        // default to 8-N-1, buffer 14
        self.operator.acquire().config(self.clock_freq, Config{
            baud_rate: 38400,
            data_bits: DataBits::Eight,
            parity: Parity::Disable,
            stop_bits: StopBit::One,
            rcvr_length: RCVRLength::Fourteen,
        })
    }

    fn terminate(&self) {
        // Do Nothing.
    }

    fn handle_int(&self) -> Result<(), ErrorNum> {
        let operator = self.operator.acquire();
        match operator.read_int_cause()? {
            IntStatus::ModemStatus => unimplemented!("Not enabled."),
            IntStatus::THREmpty => operator.dump_w_buffer(&mut self.buffer_w.acquire()),
            IntStatus::RecvAvail => operator.deplete_r_buffer(&mut self.buffer_r.acquire()),
            IntStatus::RecvLineStatus => unimplemented!("Not enabled."),
            IntStatus::TimeOut => operator.deplete_r_buffer(&mut self.buffer_r.acquire()),
        }
        Ok(())
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn as_driver<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn Driver> {
        self
    }

    fn as_int_controller<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::device::device_manager::IntController>, ErrorNum> {
        Err(ErrorNum::ENOTINTC)
    }

    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        let len = data.len();
        self.write_arr(data);
        Ok(len)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        let mut res = Vec::new();
        while res.len() < length {
            res.push(self.read_byte());
        }
        Ok(res)
    }

    fn ioctl(&self, op: usize, data: Vec<u8>) -> Result<Vec<u8>, ErrorNum> {
        let op = IOCtlOp::try_from(op)?;
        let param: IOCtlParam = cast_bytes(data)?;
        let res = match (op, param) {
            (IOCtlOp::WriteByte, IOCtlParam::Write(b)) => {
                self.write_byte(b);
                IOCtlRes::Write
            },
            (IOCtlOp::ReadByte, IOCtlParam::Read) => {
                IOCtlRes::Read(self.read_byte())
            },
            (IOCtlOp::Config, IOCtlParam::Config(param)) => {
                self.operator.acquire().config(self.clock_freq, param)?;
                IOCtlRes::Config
            },
            (IOCtlOp::Sync, IOCtlParam::Sync) => {
                let operator = self.operator.acquire();
                operator.deplete_r_buffer(&mut self.buffer_r.acquire());
                operator.dump_w_buffer(&mut self.buffer_w.acquire());

                IOCtlRes::Sync
            },
            _ => {
                return Err(ErrorNum::EINVAL);
            }
        };

        let slice = unsafe{core::slice::from_raw_parts(&res as *const IOCtlRes as *const u8, size_of::<IOCtlRes>())};
        Ok(slice.to_vec())
    }
}