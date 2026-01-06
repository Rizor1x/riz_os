#![no_std]
#![feature(alloc_error_handler)] 
#![feature(abi_x86_interrupt)] 

extern crate alloc; 

// Объявляем модули с поддержкой макросов
#[macro_use] 
pub mod serial;

#[macro_use]
pub mod writer; 

pub mod memory;
pub mod allocator;
pub mod interrupts;
pub mod task;
pub mod fs;
pub mod gdt;
pub mod hypervisor;
pub mod shell;
pub mod graphics;
pub mod window;

use core::sync::atomic::{AtomicU64};

use limine::request::MemoryMapRequest;
pub static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);


pub fn init(buffer: *mut u8, pitch: u64, width: u64, height: u64, bpp: u16) {
    serial::init();
    
    // Инициализация графического движка
    {
        let mut screen = graphics::SCREEN.lock();
        *screen = Some(graphics::Screen::new(
            buffer, 
            width as usize, 
            height as usize, 
            pitch as usize
        ));
    }
    
    serial_println!("Graphics initialized (Double Buffering).");
    
    // Остальное (IDT, GDT) оставляем...
    serial_println!("Initializing GDT...");
    gdt::init();
    serial_println!("Initializing IDT...");
    interrupts::init_idt();
    crate::interrupts::init_input(); // Используем наш общий инит
}

pub fn print_memory_map(mmap_request: &MemoryMapRequest) {
    println!("--- Memory Map ---");
    
    if let Some(response) = mmap_request.get_response() {
        for entry in response.entries() {
            if entry.entry_type == limine::memory_map::EntryType::USABLE {
                println!("RAM: {:#x}, Size: {} KB", entry.base, entry.length / 1024);
            }
        }
    }
}

pub fn hcf() -> ! {
    loop {
        core::hint::spin_loop();
    }
}