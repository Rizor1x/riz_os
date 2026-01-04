#![no_std]

pub mod serial;
pub mod memory;
// Если ты уже создал writer.rs и подключил шрифты - раскомментируй следующую строку:
// pub mod writer; 

use limine::request::MemoryMapRequest;

// Если используешь writer, добавь аргументы width, height, bpp
pub fn init(buffer: *mut u8, pitch: u64, width: u64, height: u64, bpp: u16) {
    serial::init();
    serial_println!("Hello from RizOS Kernel!");

    // Рисуем синий экран (чтобы видеть, что работает)
    for y in 0..height {
        for x in 0..width {
            let pixel_offset = y * pitch + x * ((bpp as u64) / 8);
            unsafe {
                let ptr = buffer.add(pixel_offset as usize) as *mut u32;
                *ptr = 0x00000088; 
            }
        }
    }
}

pub fn print_memory_map(mmap_request: &MemoryMapRequest) {
    serial_println!("--- Memory Map ---");
    
    if let Some(response) = mmap_request.get_response() {
        for entry in response.entries() {
            // ИСПРАВЛЕНИЕ: Мы не печатаем entry.entry_type через {:?}, так как это вызывало ошибку.
            // Просто печатаем адрес и длину.
            serial_println!(
                "Start: {:#x}, Len: {:#x}", 
                entry.base, 
                entry.length
            );
            
            // Если хочешь знать тип, можно проверить вручную:
            if entry.entry_type == limine::memory_map::EntryType::USABLE {
                serial_println!("  -> This is USABLE memory");
            }
        }
    } else {
        serial_println!("Failed to get memory map!");
    }
}

pub fn hcf() -> ! {
    loop {
        core::hint::spin_loop();
    }
}