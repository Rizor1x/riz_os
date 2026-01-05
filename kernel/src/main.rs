#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest};

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

fn execute_command(command: &str) {
    let command = command.trim(); // Убираем пробелы по краям
    match command {
        "help" => {
            println!("\nRizOS Help:");
            println!("  help  - Show this message");
            println!("  ver   - Show OS version");
            println!("  echo  - Print text back");
            println!("  clear - Clear screen");
        },
        "ver" => println!("\nRizOS v0.1.0 (Async Edition)"),
        "clear" => println!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n"),
        "" => {},
        cmd => {
            // Исправленная логика для echo
            if cmd.starts_with("echo ") {
                println!("\n{}", &cmd[5..]);
            } else if cmd == "echo" {
                println!("\n(Empty echo)");
            } else {
                println!("\nUnknown command: '{}'", cmd);
            }
        }
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