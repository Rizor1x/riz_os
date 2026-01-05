#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use limine::BaseRevision;
use kernel_core::fs::TarFileSystem; // Импорт FS
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest, ModuleRequest}; // + ModuleRequest

use kernel_core::{init, hcf, serial_println, print, println};
use kernel_core::memory::BootInfoFrameAllocator;
use kernel_core::task::{Task, simple_executor::SimpleExecutor};
use kernel_core::task::keyboard::ScancodeStream;


use x86_64::VirtAddr;
use alloc::{string::String};
use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

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

#[used]
#[link_section = ".requests"]
pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref FILESYSTEM: Mutex<Option<TarFileSystem>> = Mutex::new(None);
}

fn execute_command(command: &str) {
    let command = command.trim();
    
    // Проверяем команды с аргументами
    if command.starts_with("cat ") {
        let filename = &command[4..];
        if let Some(fs) = &*FILESYSTEM.lock() {
            if let Some(data) = fs.read_file(filename) {
                // Пытаемся превратить байты в текст и напечатать
                if let Ok(text) = core::str::from_utf8(data) {
                    println!("\nContent of {}:\n----------------\n{}\n----------------", filename, text);
                } else {
                    println!("\nFile contains binary data.");
                }
            } else {
                println!("\nFile not found: {}", filename);
            }
        } else {
            println!("\nFilesystem not initialized!");
        }
        return;
    }

    match command {
        "help" => {
            println!("\nRizOS Help:");
            println!("  ls    - List files");
            println!("  cat   - Print file content (usage: cat name)");
            // ... старые команды ...
        },
        "ls" => {
            println!();
            if let Some(fs) = &*FILESYSTEM.lock() {
                println!("Files on disk.tar:");
                for (name, size) in fs.list_files() {
                    println!(" - {} ({} bytes)", name, size);
                }
            } else {
                println!("Filesystem not initialized!");
            }
        },
        // ... старые команды (ver, clear) ...
        _ => println!("\nUnknown command: '{}'", command),
    }
}

// Теперь принимаем stream как аргумент
async fn async_keyboard_task(mut scancodes: ScancodeStream) {
    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::Ignore);
    let mut command_buffer = String::new();

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => match character {
                        '\n' => {
                            execute_command(&command_buffer);
                            command_buffer.clear();
                            print!("\n> ");
                        },
                        '\x08' => {
                            if !command_buffer.is_empty() {
                                command_buffer.pop();
                                print!("{}", character);
                            }
                        },
                        _ => {
                            command_buffer.push(character);
                            print!("{}", character);
                        }
                    },
                    DecodedKey::RawKey(_) => {},
                }
            }
        }
    }
}

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

    // 1. Графика
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            init(framebuffer.addr(), framebuffer.pitch(), framebuffer.width(), framebuffer.height(), framebuffer.bpp());
        }
    }

    // 2. Память
    let hhdm_offset = HHDM_REQUEST.get_response().expect("No HHDM").offset();
    let virt_offset = VirtAddr::new(hhdm_offset);
    let memory_map = MEMORY_MAP_REQUEST.get_response().expect("No Mmap").entries();

    let mut mapper = unsafe { kernel_core::memory::init_mapper(virt_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_map) };

    kernel_core::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap failed");

    // 3. Железо
    serial_println!("Initializing Keyboard Controller...");
    kernel_core::interrupts::enable_keyboard(); 

    // --- ИНИЦИАЛИЗАЦИЯ ФАЙЛОВОЙ СИСТЕМЫ ---
    serial_println!("Loading Filesystem...");
    if let Some(response) = MODULE_REQUEST.get_response() {
        if let Some(modules) = response.modules().get(0) { // Берем первый модуль (disk.tar)
             let addr = modules.addr();
             let size = modules.size();
             
             serial_println!("Found module at {:#x}, size: {}", addr as u64, size);
             
             // Сохраняем FS в глобальную переменную
             unsafe {
                 *FILESYSTEM.lock() = Some(TarFileSystem::new(addr, size));
             }
             serial_println!("Filesystem mounted!");
        } else {
            serial_println!("Warning: No modules loaded by Limine.");
        }
    } else {
        serial_println!("Warning: Module request failed.");
    }

    // 4. Многозадачность
    let mut executor = SimpleExecutor::new();
    
    // --- ИСПРАВЛЕНИЕ ГОНКИ ---
    // Создаем очередь ДО включения прерываний
    let scancode_stream = ScancodeStream::new();
    
    // Передаем её в задачу
    executor.spawn(Task::new(async_keyboard_task(scancode_stream)));
    
    // Фоновая задача
    executor.spawn(Task::new(async {
        serial_println!("Background task started!");
    }));

    // 5. Теперь безопасно включаем прерывания
    serial_println!("Enabling Interrupts...");
    x86_64::instructions::interrupts::enable(); 

    executor.run();

    loop { x86_64::instructions::hlt(); }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("PANIC: {}", info);
    hcf();
}