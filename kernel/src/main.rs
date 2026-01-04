#![no_std]
#![no_main]

use core::panic::PanicInfo;
use limine::BaseRevision;
// 1. Объединили все запросы в одну строку
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest};

// 2. Подключаем макрос serial_println, чтобы использовать его здесь
use kernel_core::{init, print_memory_map, hcf, serial_println};

// --- ЗАПРОСЫ ---
#[used]
#[link_section = ".requests"]
pub static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[link_section = ".requests"]
pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[link_section = ".requests"]
pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[link_section = ".requests"]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Обманка для компилятора
    unsafe {
        core::ptr::read_volatile(&BASE_REVISION);
        core::ptr::read_volatile(&FRAMEBUFFER_REQUEST);
        core::ptr::read_volatile(&MEMORY_MAP_REQUEST);
        core::ptr::read_volatile(&HHDM_REQUEST);
    }

    if !BASE_REVISION.is_supported() {
        hcf();
    }

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            init(
                framebuffer.addr(),
                framebuffer.pitch(),
                framebuffer.width(),
                framebuffer.height(),
                framebuffer.bpp()
            );
        }
    }

    // 3. Исправили доступ к offset (добавили скобки)
    let hhdm_offset = HHDM_REQUEST.get_response()
        .expect("Failed to get HHDM response")
        .offset();
        
    // Теперь макрос доступен, так как мы его импортировали
    serial_println!("HHDM Offset: {:#x}", hhdm_offset);

    // Печать карты памяти
    print_memory_map(&MEMORY_MAP_REQUEST);
    
    // --- ТЕСТ АЛЛОКАТОРА (Если хочешь проверить выдачу страниц) ---
    /*
    use kernel_core::memory::BootInfoFrameAllocator;
    use x86_64::structures::paging::FrameAllocator;
    
    let memory_map = MEMORY_MAP_REQUEST.get_response()
        .expect("Failed to get memory map")
        .entries();

    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(memory_map)
    };
    
    let page1 = frame_allocator.allocate_frame();
    serial_println!("Allocated frame: {:?}", page1);
    */
    // -------------------------------------------------------------

    hcf();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // 4. Добавили _
    hcf();
}