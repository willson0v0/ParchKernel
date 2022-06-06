



use alloc::sync::Arc;


use crate::utils::{SpinMutex};
use lazy_static::*;

use crate::device::Driver;


lazy_static!{
    pub static ref K_PRINT_HANDLER: SpinMutex<KPrintHandler> = SpinMutex::new("k print", KPrintHandler{uart_driver: None});
}

pub struct KPrintHandler {
    uart_driver: Option<Arc<dyn Driver>>
}

impl KPrintHandler {
    pub fn set_driver(&mut self, driver: Arc<dyn Driver>) {
       self.uart_driver = Some(driver);
    }

    pub fn k_puts(&mut self, s: &str) {
        if let Some(driver) = self.uart_driver.clone() {
            driver.write(s.as_bytes().to_vec()).unwrap();
        }
    }
}