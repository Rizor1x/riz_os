#![no_std]
#![feature(alloc_error_handler)] 
#![feature(abi_x86_interrupt)] 

extern crate alloc; 

pub mod serial;
pub mod writer; // <--- ДОБАВИЛИ
pub mod memory;
pub mod allocator;
pub mod interrupts;

use limine::request::MemoryMapRequest;

pub fn init(buffer: *mut u8, pitch: u64, width: u64, height: u64, bpp: u16) {
    serial::init();
    
    // --- ИНИЦИАЛИЗАЦИЯ ЭКРАНА ---
    {
        let mut writer_guard = writer::WRITER.lock();
        *writer_guard = Some(writer::FrameBufferWriter::new(buffer, pitch, width, height, bpp));
    }
    
    // Очищаем экран синим цветом (используя наш новый writer, это проще)
    // Можно оставить старый цикл, но writer.clear_screen() удобнее, 
    // но он приватный, так что пока оставим ручную заливку или просто println.
    
    // Рисуем фон (как и раньше)
    for y in 0..height {
        for x in 0..width {
            let pixel_offset = y * pitch + x * ((bpp as u64) / 8);
            unsafe {
                let ptr = buffer.add(pixel_offset as usize) as *mut u32;
                *ptr = 0x00000088; 
            }
        }
    }
    
    // ТЕПЕРЬ МОЖНО ПИСАТЬ НА ЭКРАН!
    // Используем макрос println! из модуля writer (он экспортируется глобально)
    crate::println!("RizOS Kernel v0.1");
    crate::println!("Display: {}x{}", width, height);
    
    serial_println!("Graphics initialized.");
    
    serial_println!("Initializing IDT...");
    interrupts::init_idt();
    serial_println!("IDT Initialized.");

    crate::println!("\nType 'help' for commands.");
    crate::print!("> "); // Приглашение
}

pub fn print_memory_map(mmap_request: &MemoryMapRequest) {
    // Теперь мы можем печатать карту памяти НА ЭКРАН!
    crate::println!("--- Memory Map ---"); 
    
    if let Some(response) = mmap_request.get_response() {
        for entry in response.entries() {
             if entry.entry_type == limine::memory_map::EntryType::USABLE {
                crate::println!("RAM: {:#x}, Size: {} KB", entry.base, entry.length / 1024);
             }
        }
    }
}

pub fn hcf() -> ! {
    loop {
        core::hint::spin_loop();
    }
}