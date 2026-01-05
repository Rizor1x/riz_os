#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest, ModuleRequest};

use kernel_core::{init, hcf, serial_println, print, println};
use kernel_core::memory::BootInfoFrameAllocator;
use kernel_core::task::{Task, simple_executor::SimpleExecutor};
use kernel_core::task::keyboard::ScancodeStream;
use kernel_core::task::mouse::MouseStream;
use kernel_core::fs::TarFileSystem;

use x86_64::VirtAddr;
use alloc::string::String;
use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

use spin::Mutex;
use lazy_static::lazy_static;

// --- REQUESTS ---
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

lazy_static! {
    static ref FILESYSTEM: Mutex<Option<TarFileSystem>> = Mutex::new(None);
}

// --- SHELL ---

fn execute_command(command: &str) {
    let command = command.trim();
    if command.starts_with("cat ") {
        let filename = &command[4..];
        if let Some(fs) = &*FILESYSTEM.lock() {
            if let Some(data) = fs.read_file(filename) {
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
            println!("  help  - Show this message");
            println!("  ver   - Show OS version");
            println!("  ls    - List files");
            println!("  cat   - Read file");
            println!("  clear - Clear screen");
            println!("  cpu   - CPU Info");
        },
        "ver" => println!("\nRizOS v0.1.0"),
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
        "clear" => println!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n"),
        "cpu" => {
            use raw_cpuid::CpuId;
            let cpuid = CpuId::new();
            if let Some(v) = cpuid.get_vendor_info() { println!("Vendor: {}", v.as_str()); }
            if let Some(f) = cpuid.get_feature_info() {
                if f.has_vmx() { println!("[+] VMX Supported!"); } else { println!("[-] VMX Not Supported"); }
            }
        },
        "" => {},
        _ => println!("\nUnknown command: '{}'", command),
    }
}

// --- TASKS ---

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

// ИСПРАВЛЕНО: Добавлены аргументы width и height
async fn async_mouse_task(mut stream: MouseStream, width: usize, height: usize) {
    let mut x = width / 2;
    let mut y = height / 2;
    
    let mut packet = [0u8; 3];
    let mut packet_idx = 0;

    // ЭКСПЕРИМЕНТИРУЙ С ЭТИМ ЧИСЛОМ! 
    // Попробуй 4, 5 или 6. Чем больше - тем меньше придется махать мышкой.
    const MOUSE_SPEED: i32 = 1; 

    while let Some(byte) = stream.next().await {
        match packet_idx {
            0 => {
                if (byte & 0x08) == 0 { continue; }
                packet[0] = byte;
                packet_idx = 1;
            }
            1 => { packet[1] = byte; packet_idx = 2; }
            2 => {
                packet[2] = byte;
                packet_idx = 0;

                let _header = packet[0];
                
                // Преобразуем u8 -> i8 -> i32 (чтобы корректно обрабатывать отрицательные числа)
                let dx_raw = (packet[1] as i8) as i32;
                let dy_raw = (packet[2] as i8) as i32;

                // --- ПРОСТАЯ АКСЕЛЕРАЦИЯ ---
                // Если мышь двигается быстро, умножаем сильнее
                let speed_mult = if dx_raw.abs() > 5 || dy_raw.abs() > 5 { 2 } else { 1 };
                
                let dx = dx_raw * MOUSE_SPEED * speed_mult;
                let dy = dy_raw * MOUSE_SPEED * speed_mult;

                // Обновляем X
                let new_x = x as i32 + dx;
                x = new_x.clamp(0, (width - 5) as i32) as usize;

                // Обновляем Y (инверсия)
                let new_y = y as i32 - dy;
                y = new_y.clamp(0, (height - 5) as i32) as usize;

                kernel_core::writer::update_mouse_cursor(x, y);
            }
            _ => unreachable!(),
        }
    }
}

// --- ENTRY POINT ---

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        core::ptr::read_volatile(&BASE_REVISION);
        core::ptr::read_volatile(&FRAMEBUFFER_REQUEST);
        core::ptr::read_volatile(&MEMORY_MAP_REQUEST);
        core::ptr::read_volatile(&HHDM_REQUEST);
        core::ptr::read_volatile(&MODULE_REQUEST);
    }

    if !BASE_REVISION.is_supported() {
        hcf();
    }

    // Сохраняем размеры экрана
    let mut screen_width = 1024;
    let mut screen_height = 768;

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            screen_width = framebuffer.width() as usize;
            screen_height = framebuffer.height() as usize;
            
            init(
                framebuffer.addr(),
                framebuffer.pitch(),
                framebuffer.width(),
                framebuffer.height(),
                framebuffer.bpp()
            );
        }
    }

    // Init Memory
    let hhdm_offset = HHDM_REQUEST.get_response().expect("No HHDM").offset();
    let virt_offset = VirtAddr::new(hhdm_offset);
    let memory_map = MEMORY_MAP_REQUEST.get_response().expect("No Mmap").entries();

    let mut mapper = unsafe { kernel_core::memory::init_mapper(virt_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_map) };

    kernel_core::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap failed");

    // Load FS
    serial_println!("Loading Filesystem...");
    if let Some(response) = MODULE_REQUEST.get_response() {
        if let Some(module) = response.modules().get(0) {
            let addr = module.addr();
            let size = module.size();
            unsafe {
                *FILESYSTEM.lock() = Some(TarFileSystem::new(addr, size));
            }
            serial_println!("Filesystem mounted!");
        }
    }

    // Init Devices
    serial_println!("Initializing Input...");
    kernel_core::interrupts::enable_keyboard(); 
    kernel_core::interrupts::enable_mouse();

    // Start Tasks
    let mut executor = SimpleExecutor::new();
    let scancode_stream = ScancodeStream::new();
    let mouse_stream = MouseStream::new();

    executor.spawn(Task::new(async_keyboard_task(scancode_stream)));
    
    // ИСПРАВЛЕНО: Передаем width и height
    executor.spawn(Task::new(async_mouse_task(mouse_stream, screen_width, screen_height)));

    serial_println!("Starting OS...");
    x86_64::instructions::interrupts::enable(); 

    executor.run();

    loop { x86_64::instructions::hlt(); }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("PANIC: {}", info);
    hcf();
}