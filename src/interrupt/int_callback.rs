use crate::utils::ErrorNum;


pub trait IntCallback {
    fn get_irq_num(&self) -> u32;
    fn handle_int(&self) -> Result<u32, ErrorNum>;
}