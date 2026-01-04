#![no_std]
#![no_main]
#![feature(alloc_error_handler)] // Тоже нужно тут

extern crate alloc; // Нужно, чтобы использовать макрос vec!

use core::panic::PanicInfo;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest};
use kernel_core::{init, hcf, serial_println};

// Импорты для памяти
use kernel_core::memory::BootInfoFrameAllocator;
use x86_64::VirtAddr;
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc}; // Импортируем типы

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

    // 1. Инициализация Маппера и Аллокатора
    
    // Получаем offset
    let hhdm_offset = HHDM_REQUEST.get_response()
        .expect("Failed to get HHDM").offset();
    let virt_offset = VirtAddr::new(hhdm_offset);
    
    // Получаем карту памяти
    let memory_map = MEMORY_MAP_REQUEST.get_response()
        .expect("Failed to get mmap")
        .entries(); // Это возвращает &[&Entry]

    // Инициализируем маппер (управление таблицами страниц)
    let mut mapper = unsafe { kernel_core::memory::init_mapper(virt_offset) };
    
    // Инициализируем Frame Allocator (выдача физических страниц)
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(memory_map)
    };

    // Инициализируем КУЧУ!
    serial_println!("Initializing Heap...");
    kernel_core::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Heap initialization failed");

    serial_println!("Heap Initialized! Testing...");

    // 2. ТЕСТЫ КУЧИ
    
    // Тест 1: Box (простое выделение)
    let heap_value = Box::new(41);
    serial_println!("heap_value at {:p}", heap_value);

    // Тест 2: Vec (динамический массив)
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    serial_println!("Vec created. Length: {}", vec.len());
    serial_println!("Vec[100] = {}", vec[100]); // Должно быть 100

    // Тест 3: Reference Counting (сложное выделение)
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    serial_println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    
    serial_println!("All Heap tests passed!");

    hcf();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("{}", info); // Теперь печатаем панику в лог!
    hcf();
}