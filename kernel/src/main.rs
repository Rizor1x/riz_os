#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest, ModuleRequest, MpRequest};

use kernel_core::{init, hcf, serial_println, println, print};
use kernel_core::memory::BootInfoFrameAllocator;
use kernel_core::fs::TarFileSystem;
use kernel_core::graphics::SCREEN;
use kernel_core::window::{Window, WindowManager};

use x86_64::VirtAddr;
use core::sync::atomic::Ordering;

// Импортируем флаги управления
use kernel_core::interrupts::{STOP_VM, VM_ACTIVE};

use kernel_core::task::{Task, simple_executor::SimpleExecutor};
use kernel_core::task::keyboard::ScancodeStream;
use kernel_core::task::mouse::MouseStream;
use kernel_core::hypervisor;

#[used] #[link_section = ".requests"] pub static BASE_REVISION: BaseRevision = BaseRevision::new();
#[used] #[link_section = ".requests"] pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();
#[used] #[link_section = ".requests"] pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
#[used] #[link_section = ".requests"] pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
#[used] #[link_section = ".requests"] pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();
#[used] #[link_section = ".requests"] pub static MP_REQUEST: MpRequest = MpRequest::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        core::ptr::read_volatile(&BASE_REVISION);
        core::ptr::read_volatile(&FRAMEBUFFER_REQUEST);
        core::ptr::read_volatile(&MEMORY_MAP_REQUEST);
        core::ptr::read_volatile(&HHDM_REQUEST);
        core::ptr::read_volatile(&MODULE_REQUEST);
        core::ptr::read_volatile(&MP_REQUEST);
    }
    if !BASE_REVISION.is_supported() { hcf(); }

    // 1. Память
    let hhdm_offset = HHDM_REQUEST.get_response().expect("No HHDM").offset();
    kernel_core::HHDM_OFFSET.store(hhdm_offset, Ordering::Relaxed);
    let virt_offset = VirtAddr::new(hhdm_offset);
    let memory_map = MEMORY_MAP_REQUEST.get_response().expect("No Mmap").entries();
    let mut mapper = unsafe { kernel_core::memory::init_mapper(virt_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_map) };
    kernel_core::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap failed");

    // 2. Графика
    let mut screen_width = 1024;
    let mut screen_height = 768;
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            screen_width = framebuffer.width() as usize;
            screen_height = framebuffer.height() as usize;
            init(framebuffer.addr(), framebuffer.pitch(), framebuffer.width(), framebuffer.height(), framebuffer.bpp());
        }
    }

    // 3. Файлы
    if let Some(response) = MODULE_REQUEST.get_response() {
        if let Some(module) = response.modules().get(0) {
             let addr = module.addr();
             let size = module.size();
             unsafe { *kernel_core::shell::FILESYSTEM.lock() = Some(TarFileSystem::new(addr, size)); }
             serial_println!("FS Mounted");
        }
    }

    // 4. Ввод
    kernel_core::interrupts::init_input(); 
    let mut executor = SimpleExecutor::new();
    // Мы создаем потоки, но пока они не используются, так как шелл работает через прерывания
    let _scancode_stream = ScancodeStream::new();
    let _mouse_stream = MouseStream::new();

    serial_println!("Enabling Interrupts...");
    x86_64::instructions::interrupts::enable(); 

    // --- GUI ---
    let mut wm = WindowManager::new();
    wm.add(Window::new(0, 50, 50, 700, 500, "Terminal", 0xFF000088));
    wm.add(Window::new(1, 800, 100, 300, 200, "VM Status", 0xFF444444));

    println!("Welcome to RizOS GUI!");
    println!("Type 'help' for commands.");
    print!("> ");

    // Локальная переменная: инициализирована ли виртуалка?
    let mut vm_initialized = false;

    // --- ГЛАВНЫЙ ЦИКЛ ---
    loop {
        // 1. Управление Виртуалкой
        let vm_active = VM_ACTIVE.load(Ordering::Relaxed);

        if vm_active {
            // Если включили, но еще не инициализировали -> Инициализация
            if !vm_initialized {
                unsafe {
                    if let Ok(_) = crate::hypervisor::init_vm() {
                        vm_initialized = true;
                        println!("\n[System] VM Hypervisor Initialized.");
                    } else {
                        println!("\n[System] VM Init Failed! Turning off.");
                        VM_ACTIVE.store(false, Ordering::Relaxed);
                    }
                }
            } else {
                // Если инициализировано -> Крутим виртуалку (один тик)
                unsafe {
                    if !crate::hypervisor::tick_vm() {
                        // Ошибка внутри VM
                        VM_ACTIVE.store(false, Ordering::Relaxed);
                        vm_initialized = false;
                        crate::hypervisor::stop_vm();
                    }
                }
            }

            // Проверка выхода (ESC)
            if STOP_VM.load(Ordering::Relaxed) {
                println!("\n[System] Stopping VM by user request.");
                VM_ACTIVE.store(false, Ordering::Relaxed);
                STOP_VM.store(false, Ordering::Relaxed);
                
                unsafe {
                    if vm_initialized {
                        crate::hypervisor::stop_vm();
                        vm_initialized = false;
                    }
                }
            }
        }

        // 2. GUI и Ввод
        executor.run_ready_tasks();

        x86_64::instructions::interrupts::without_interrupts(|| {
            let mx = unsafe { kernel_core::interrupts::MOUSE_X as usize }.clamp(0, screen_width - 10);
            let my = unsafe { kernel_core::interrupts::MOUSE_Y as usize }.clamp(0, screen_height - 10);
            let lmb = unsafe { kernel_core::interrupts::MOUSE_LEFT_PRESSED };

            wm.update_mouse(mx, my, lmb);

            if let Some(screen) = &mut *SCREEN.lock() {
                screen.clear(0x00224466);
                
                // РИСУЕМ СЛОИ (Z-ORDER)
                for window in wm.iter() {
                    // 1. Рисуем само окно
                    window.draw(screen);
                    
                    // 2. Если это Терминал (ID 0) — рисуем текст СРАЗУ
                    if window.id == 0 {
                        if let Some(console) = kernel_core::writer::CONSOLE.try_lock() {
                            console.draw(screen, window.x + 10, window.y + 35);
                        }
                    }
                }
                
                // --- НОВОЕ: ИНДИКАТОР VM ---
                // Рисуем панель статуса внизу
                screen.fill_rect(0, screen_height - 30, screen_width, 30, 0xFF333333);
                
                // Лампочка
                let status_color = { if vm_active { 0xFF00FF00 } else { 0xFFFF0000 } };
                screen.fill_rect(screen_width - 25, screen_height - 25, 20, 20, status_color);

                kernel_core::writer::draw_mouse(screen, mx, my);
                screen.present();
            }
        });

        // Пауза, чтобы не грузить CPU (если VM не работает)
        if !vm_active {
            for _ in 0..50000 { core::hint::spin_loop(); }
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("PANIC: {}", info);
    hcf();
}