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

use limine::request::MemoryMapRequest;

pub fn init(buffer: *mut u8, pitch: u64, width: u64, height: u64, bpp: u16) {
    // 1. Сначала Serial, чтобы работали логи
    serial::init();
    
    // 2. Инициализация экрана
    {
        let mut writer_guard = writer::WRITER.lock();
        *writer_guard = Some(writer::FrameBufferWriter::new(buffer, pitch, width, height, bpp));
    }
    
    // Очистка экрана
    for y in 0..height {
        for x in 0..width {
            let pixel_offset = y * pitch + x * ((bpp as u64) / 8);
            unsafe {
                let ptr = buffer.add(pixel_offset as usize) as *mut u32;
                *ptr = 0x00000088; 
            }
        }
    }
    
    println!("RizOS Kernel v0.1");
    println!("Display: {}x{}", width, height);
    serial_println!("Graphics initialized.");
    
    // --- ИСПРАВЛЕНИЕ: ПОРЯДОК ИНИЦИАЛИЗАЦИИ ---
    
    // 3. СНАЧАЛА GDT (Фундамент)
    // Мы должны настроить сегменты и TSS до того, как создадим IDT.
    serial_println!("Initializing GDT...");
    gdt::init();
    serial_println!("GDT Initialized.");

    // 4. ПОТОМ IDT (Прерывания)
    // Теперь IDT захватит правильные сегменты из нашей GDT.
    serial_println!("Initializing IDT...");
    interrupts::init_idt();
    serial_println!("IDT Initialized.");

    // ------------------------------------------

    println!("\nShell ready.");
    print!("> ");
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