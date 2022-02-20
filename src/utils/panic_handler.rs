use core::panic::PanicInfo;


#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        fatal!("Panic @ {}:{} : {}", location.file(), location.line(), info.message().unwrap());
    } else {
        fatal!("Panic @ ?:? : {}", info.message().unwrap());
    }
    loop {}
}