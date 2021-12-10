#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use alloc::sync;
use crate::utils::{SpinMutex, Mutex};
use super::{VirtAddr, PhysAddr};
use core::option::Option;
use crate::interrupt::get_cpu;

// |--------|---------|-------------------------------------------------|
// | Bit    | Pattern | Meaning                                         |
// |--------|---------|-------------------------------------------------|
// | 7&6    | 00      | No FIFO on chip                                 |
// |        | 01      | Reserved condition                              |
// |        | 10      | FIFO enabled, but not functioning               |
// |        | 11      | FIFO enabled                                    |
// |--------|---------|-------------------------------------------------|
// | 5&4    | /       | Reserved                                        |
// |--------|---------|-------------------------------------------------|
// | 3&2&1  | 000     | Modem Status Interrupt                          |
// |        | 000     | Transmitter Holding Register Empty Interrupt    |
// |        | 000     | Received Data Available Interrupt               |
// |        | 000     | Receiver Line Status Interrupt                  |
// |        | 000     | Reserved                                        |
// |        | 000     | Reserved                                        |
// |        | 000     | Time-out Interrupt Pending (16550 & later)      |
// |        | 000     | Reserved                                        |
// |--------|---------|-------------------------------------------------|
// | 0      | /       | Interrupt Pending Flag                          |
// |--------|---------|-------------------------------------------------|

#[repr(u8)]
#[derive(Debug)]
pub enum IIRFIFOStatus {
    NoFIFO      = 0b00000000,
    EnabledFifo = 0b10000000,
    BadFifo     = 0b11000000,
}

impl core::convert::TryFrom<u8> for IIRFIFOStatus {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == (IIRFIFOStatus::NoFIFO        as u8) & 0b11000000 => Ok(IIRFIFOStatus::NoFIFO        ),
            x if x == (IIRFIFOStatus::EnabledFifo   as u8) & 0b11000000 => Ok(IIRFIFOStatus::EnabledFifo   ),
            x if x == (IIRFIFOStatus::BadFifo       as u8) & 0b11000000 => Ok(IIRFIFOStatus::BadFifo       ),
            _ => Err(()),
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
pub enum IIRIntStatus {
    ModemStatus     = 0b00000000,
    THREmpty        = 0b00001000,
    RecvAvail       = 0b00010000,
    RecvLineStatus  = 0b00011000,
    TimeOut         = 0b00110000,
}

impl core::convert::TryFrom<u8> for IIRIntStatus {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == (IIRIntStatus::ModemStatus       as u8) & 0b00111000 => Ok(IIRIntStatus::ModemStatus      ),
            x if x == (IIRIntStatus::THREmpty          as u8) & 0b00111000 => Ok(IIRIntStatus::THREmpty         ),
            x if x == (IIRIntStatus::RecvAvail         as u8) & 0b00111000 => Ok(IIRIntStatus::RecvAvail        ),
            x if x == (IIRIntStatus::RecvLineStatus    as u8) & 0b00111000 => Ok(IIRIntStatus::RecvLineStatus   ),
            x if x == (IIRIntStatus::TimeOut           as u8) & 0b00111000 => Ok(IIRIntStatus::TimeOut          ),
            _ => Err(()),
        }
    }
}

// | Base Address | DLAB | I/O Access | Abbrv. | Register Name                     |
// | ------------ | ---- | ---------- | ------ | --------------------------------- |
// | +0           | 0    | Write      | THR    | Transmitter Holding Buffer        |
// | +0           | 0    | Read       | RBR    | Receiver Buffer                   |
// | +0           | 1    | Read/Write | DLL    | Divisor Latch Low Byte            |
// | +1           | 0    | Read/Write | IER    | Interrupt Enable Register         |
// | +1           | 1    | Read/Write | DLH    | Divisor Latch High Byte           |
// | +2           | x    | Read       | IIR    | Interrupt Identification Register |
// | +2           | x    | Write      | FCR    | FIFO Control Register             |
// | +3           | x    | Read/Write | LCR    | Line Control Register             |
// | +4           | x    | Read/Write | MCR    | Modem Control Register            |
// | +5           | x    | Read       | LSR    | Line Status Register              |
// | +6           | x    | Read       | MSR    | Modem Status Register             |
// | +7           | x    | Read/Write | SR     | Scratch Register                  |
struct UartInner{
    transmitter_holding_buffer          : PhysAddr,
    receiver_buffer                     : PhysAddr,
    divisor_latch_low_byte              : PhysAddr,
    interrupt_enable_register           : PhysAddr,
    divisor_latch_high_byte             : PhysAddr,
    interrupt_identification_register   : PhysAddr,
    fifo_control_register               : PhysAddr,
    line_control_register               : PhysAddr,
    modem_control_register              : PhysAddr,
    line_status_register                : PhysAddr,
    modem_status_register               : PhysAddr,
    scratch_register                    : PhysAddr,
    write_buffer                        : VecDeque<u8>,
    read_buffer                         : VecDeque<u8>
}

pub struct Uart {
    address: usize,
    inner: SpinMutex<UartInner>
}

impl Uart {
    pub fn new(address: usize) -> Self {
        let inner = UartInner{
            transmitter_holding_buffer          : (address + 0x0).into(),
            receiver_buffer                     : (address + 0x0).into(),
            divisor_latch_low_byte              : (address + 0x0).into(),
            interrupt_enable_register           : (address + 0x1).into(),
            divisor_latch_high_byte             : (address + 0x1).into(),
            interrupt_identification_register   : (address + 0x2).into(),
            fifo_control_register               : (address + 0x2).into(),
            line_control_register               : (address + 0x3).into(),
            modem_control_register              : (address + 0x4).into(),
            line_status_register                : (address + 0x5).into(),
            modem_status_register               : (address + 0x6).into(),
            scratch_register                    : (address + 0x7).into(),
            write_buffer                        : VecDeque::new(),
            read_buffer                         : VecDeque::new()
        };
        inner.init(115200, 38400);
        Self {
            address,
            inner: SpinMutex::new("Uart Lock".to_string(), inner)
        }
    }

    /// Write to UART, using it's interrupt
    pub fn write(&self, data: String) {
        let mut inner = self.inner.acquire();
        while inner.write_buffer.len() >= 1024 {
            // TODO: drop yield lock
        }
        inner.write(data);
        inner.sync();
    }

    /// kernel will use this to send output
    pub fn write_synced(&self, data: String) {
        self.inner.acquire().write_synced(data);
    }

    /// Read from UART
    pub fn read(&self) -> char {
        let mut inner = self.inner.acquire();
        while inner.read_buffer.len() == 0 {
            // TODO: drop yield lock
        }
        let init : u8 = inner.read_buffer.pop_front().unwrap();
        let mut buf : u32;

        let length : u8;
        if init < 0b10000000 {
            return init as char;
        }
        else if init < 0b11100000 {length = 2;}
        else if init < 0b11110000 {length = 3;}
        else if init < 0b11111000 {length = 4;}
        else if init < 0b11111100 {length = 5;}
        else if init < 0b11111110 {length = 6;}
        else { return '�'; }     // illegal utf-8 sequence
        buf = (init & (0b01111111 >> length)) as u32;
    
        for _i in 1..length {
            while inner.read_buffer.len() == 0 {
                // TODO: drop yield lock
            }
            let b : u8 = inner.read_buffer.pop_front().unwrap();

            if b & 0b11000000 != 0b10000000 { return '�'; }
            assert_eq!(b & 0b11000000, 0b10000000); // check utf-8 sequence
            buf <<= 6;
            buf += (b & 0b00111111) as u32;
        }
        
        match char::from_u32(buf) {
            None => '�',    // unknown sequence
            Some(res) => res
        }
    }
}

impl UartInner {
    pub fn write(&mut self, data: String) {
        for b in data.as_bytes() {
            self.write_buffer.push_back(*b);
        }
    }
    
    pub fn write_synced(&self, data: String) {
        get_cpu().acquire().push_intr_off();
        for b in data.as_bytes() {
            while self.read_reg(self.line_status_register) & 0b00100000 == 0 {}
            self.write_reg(self.transmitter_holding_buffer, *b);
        }
        get_cpu().acquire().pop_intr_off();
    }

    pub fn read(&mut self) -> Option<u8> {
        if(self.read_reg(self.line_status_register) & 0b00000001 != 0) {
            Some(self.read_reg(self.receiver_buffer))
        } else {
            None
        }
    }

    pub fn sync(&mut self) {
        while let Some(b) = self.read() {
            self.read_buffer.push_back(b);
        }

        while let Some(b) = self.write_buffer.front(){
            if (self.read_reg(self.line_status_register) & 0b00100000 == 0) {
                // UART THR is full.
                // wait for next uart interrupt.
                // TODO: Wakeup yielded process
                return;
            }

            self.write_reg(self.transmitter_holding_buffer, *b);
            self.write_buffer.pop_front();
        }
        // TODO: Wakeup yielded process
    }

    pub fn init(&self, clock_freq: usize, baud_rate: usize) {
        let divisor = clock_freq / (16 * baud_rate);
        // enable divisor latch access
        self.write_reg(self.line_control_register, 0b10000000);
        // divisor latch lower
        self.write_reg(self.divisor_latch_low_byte, (divisor & 0xFF) as u8);
        // divisor latch higher
        self.write_reg(self.divisor_latch_high_byte, ((divisor >> 8) & 0xFF) as u8);
        // disable divisor latch, 8 bit, no parity, 1 stop bit
        self.write_reg(self.line_control_register, 0b00000011);
        // reset and enable fifo
        self.write_reg(self.fifo_control_register, 0b00000111);
        // enable rx/tx interrupt
        self.write_reg(self.interrupt_enable_register, 0b00000011);
    }

    pub fn write_reg(&self, addr: PhysAddr, data: u8) {
        if  addr == self.transmitter_holding_buffer ||
            addr == self.divisor_latch_low_byte     ||
            addr == self.interrupt_enable_register  ||
            addr == self.divisor_latch_high_byte    ||
            addr == self.fifo_control_register      ||
            addr == self.line_control_register      ||
            addr == self.modem_control_register     ||
            addr == self.scratch_register  {
            unsafe {
                addr.write_volatile(&data)
            }
        } else {
            panic!("Writing to unwritable register in UART.")
        }
    }

    pub fn read_reg(&self, addr: PhysAddr) -> u8 {
        if  addr == self.receiver_buffer                    ||
            addr == self.divisor_latch_low_byte             ||
            addr == self.interrupt_enable_register          ||
            addr == self.divisor_latch_high_byte            ||
            addr == self.interrupt_identification_register  ||
            addr == self.line_control_register              ||
            addr == self.modem_control_register             ||
            addr == self.line_status_register               ||
            addr == self.modem_status_register              ||
            addr == self.scratch_register {
            unsafe {
                addr.read_volatile()
            }
        } else {
            panic!("Reading from unreadable register in UART.")
        }
    }
}