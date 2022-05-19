use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::device::DeviceTree;
use crate::mem::PhysAddr;
use crate::process::get_hart_id;
use crate::utils::{ErrorNum, Mutex, RWLock, SpinMutex, UUID};
use crate::device::device_manager::{Driver, IntController};
use core::fmt::Debug;
use core::mem::size_of;

enum_with_tryfrom_usize!{
    #[repr(usize)]
    pub enum IOCtlOp {
        SetIRQPriority = 1,
        SetHartIRQAvailability = 2,
        SetHartIntThreshold = 3,
        ReadHartIntThreshold = 4,
    }
}

pub enum IOCtlParam {
    /// irq, priority
    SetIRQPriority(u32, u32),
    /// hartid, irq, enable
    SetHartIRQAvailability(usize, u32, bool),
    /// hartid, threshold
    SetHartIntThreshold(usize, u32),
    ReadHartIntThreshold(usize)
}

pub enum IOCtlRes {
    SetIRQPriority,
    SetHartIRQAvailability,
    SetHartIntThreshold,
    ReadHartIntThreshold(u32)
}

pub struct PLIC {
    base_address: PhysAddr,
    dev_tree: DeviceTree,
    operator: SpinMutex<PLICOperator>
}

impl Debug for PLIC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PLIC @ {:?}", self.base_address)
    }
}

struct PLICOperator {
    base_address: PhysAddr
}

impl PLICOperator {
    fn irq_priority_reg(&self, irq: u32) -> PhysAddr {
        self.base_address + irq as usize * size_of::<u32>()
    }

    fn hart_irq_s_enable_reg(&self, hart: usize) -> PhysAddr {
        self.base_address + 0x2080usize + hart * 0x100usize
    }

    fn hart_irq_s_threshold_reg(&self, hart: usize) -> PhysAddr { 
        self.base_address + 0x201000usize + hart * 0x2000usize
    }

    fn hart_claim_reg(&self, hart: usize) -> PhysAddr {
        self.base_address + 0x201004usize + hart * 0x2000usize
    }

    pub fn set_irq_priority(&self, irq: u32, priority: u32) {
        // sanity check
        assert!(irq < 32);
        
        unsafe{self.irq_priority_reg(irq).write_volatile(&priority)}
    }

    pub fn hart_irq_availability(&self, hart: usize, irq: u32, availability: bool) {
        // sanity check
        assert!(irq < 32);
        assert!(hart < 16);

        // WAR dependency is ok, for the whole PLICOperator will be locked.
        let mut original: u32 = unsafe{self.hart_irq_s_enable_reg(hart).read_volatile()};
        if availability {
            original |= 1 << irq;
        } else {
            original &= !(1 << irq);
        }
        unsafe {self.hart_irq_s_enable_reg(hart).write_volatile(&original);}
    }

    pub fn set_hart_priority_threshold(&self, hart: usize, threshold: u32) {
        unsafe{self.hart_irq_s_threshold_reg(hart).write_volatile(&threshold);}
    }

    /// WAR harzard warning: The data lose it's credit once PLICOperator lock is droped.
    pub fn read_hart_priority_threshold(&self, hart: usize) -> u32 {
        unsafe{self.hart_irq_s_threshold_reg(hart).read_volatile()}
    }

    pub fn claim_hart_interrupt(&self, hart: usize) -> u32 {
        unsafe{self.hart_claim_reg(hart).read_volatile()}
    }

    pub fn complete_hart_interrupt(&self, hart: usize, irq: u32) {
        unsafe{self.hart_claim_reg(hart).write_volatile(&irq)}
    }
}

impl Driver for PLIC {
    fn new(dev_tree: crate::device::DeviceTree) -> Result<alloc::vec::Vec<(crate::utils::UUID, alloc::sync::Arc<dyn Driver>)>, crate::utils::ErrorNum> where Self: Sized {
        match dev_tree.serach_compatible("riscv,plic0")?.as_slice() {
            [node_guard] => {
                let node = node_guard.acquire_r();
                let uuid = node.driver;
                verbose!("Creating driver instance for plic, unit name {}, uuid {}", node.unit_name, uuid);
                // TODO: implement mem access guard?
                let base_address: PhysAddr = node.reg_value()?[0].address.into();
                // sanity check
                assert!(node.get_value("interrupt-controller").is_ok(), "PLIC is not interrupt controller!?");
                let res = PLIC {
                    base_address, 
                    dev_tree: dev_tree.clone(),
                    operator: SpinMutex::new("plic", PLICOperator{base_address})
                };
                return Ok(vec![(uuid, Arc::new(res))]);
            },
            _ => panic!("No plic or multiple plic in dev_tree")
        };
    }

    fn initialize(&self) -> Result<(), crate::utils::ErrorNum> {
        // default to enable all irq
        let int_dev = self.dev_tree.contains_field("interrupt")?;
        let operator = self.operator.acquire();
        let hart_count = self.dev_tree.hart_count();
        for node in int_dev.iter() {
            let int_irq = node.acquire_r().get_value("interrupt").unwrap().get_u32().unwrap();
            operator.set_irq_priority(int_irq, 1);
            // default to enable all hart
            for hart in 0..hart_count {
                operator.hart_irq_availability(hart, int_irq, true);
            }
        }
        // set threshold to 0 so all irq are enabled
        for hart in 0..hart_count {
            operator.set_hart_priority_threshold(hart, 0);
        }
        Ok(())
    }

    fn terminate(&self) {
        // do nothing
    }

    fn ioctl(&self, op: usize, data: alloc::boxed::Box<dyn core::any::Any>) -> Result<alloc::boxed::Box<dyn core::any::Any>, crate::utils::ErrorNum> {
        let op: IOCtlOp = op.try_into()?;
        let param: Box<IOCtlParam> = data.downcast().map_err(|_| ErrorNum::EINVAL)?;
        match (op, *param) {
            (IOCtlOp::SetIRQPriority, IOCtlParam::SetIRQPriority(irq, priority)) => {
                self.operator.acquire().set_irq_priority(irq, priority);
                Ok(Box::new(IOCtlRes::SetIRQPriority))
            },
            (IOCtlOp::SetHartIRQAvailability, IOCtlParam::SetHartIRQAvailability(hart, irq, availability)) => {
                self.operator.acquire().hart_irq_availability(hart, irq, availability);
                Ok(Box::new(IOCtlRes::SetHartIRQAvailability))
            },
            (IOCtlOp::SetHartIntThreshold, IOCtlParam::SetHartIntThreshold(hart, threshold)) => {
                self.operator.acquire().set_hart_priority_threshold(hart, threshold);
                Ok(Box::new(IOCtlRes::SetHartIntThreshold))
            },
            (IOCtlOp::ReadHartIntThreshold, IOCtlParam::ReadHartIntThreshold(hart)) => {
                Ok(Box::new(IOCtlRes::ReadHartIntThreshold(self.operator.acquire().read_hart_priority_threshold(hart))))
            },
            _ => Err(ErrorNum::EINVAL),
        }
    }

    fn handle_int(&self) -> Result<(), crate::utils::ErrorNum> {
        panic!("Plic won't emit interrupt for itself")
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn as_driver<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn Driver> {
        self
    }

    fn as_int_controller<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::device::device_manager::IntController>, ErrorNum> {
        Ok(self)
    }
    
    fn write(&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, _length: usize) -> Result<Vec<u8>, ErrorNum> {
        Err(ErrorNum::EPERM)
    }
}

impl IntController for PLIC {
    fn clear_int(&self, int_num: u32) -> Result<(), ErrorNum> {
        self.operator.acquire().complete_hart_interrupt(get_hart_id(), int_num);
        Ok(())
    }

    fn claim_int(&self) -> Result<u32, ErrorNum> {
        Ok(self.operator.acquire().claim_hart_interrupt(get_hart_id()))
    }
}
