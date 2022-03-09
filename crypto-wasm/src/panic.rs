use core::panic::PanicInfo;
use core::alloc::Layout;
use core::intrinsics::abort;

use crate::log::*;
use crate::error;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        error!("System Panic! {:?}", s);
    } else {
        error!("System Panic! Unknown Error has occured.");
    }

    unsafe {
        abort();
    }
}

#[no_mangle]
#[alloc_error_handler]
pub extern "C" fn oom(mem: Layout) -> ! {
    error!("System Out of Memory, wanted minimum {:?} bytes", mem.size());

    unsafe {
        abort();
    }
}
