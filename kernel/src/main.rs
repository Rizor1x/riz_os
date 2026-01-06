#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use limine::BaseRevision;
use limine::request::{FramebufferRequest, MemoryMapRequest, HhdmRequest, ModuleRequest};

use kernel_core::{init, hcf, serial_println, println, print};
use kernel_core::memory::BootInfoFrameAllocator;
use kernel_core::fs::TarFileSystem;
use kernel_core::graphics::SCREEN;
use kernel_core::window::Window;
use alloc::vec;

use x86_64::VirtAddr;
use core::sync::atomic::Ordering;

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
#[used]
#[link_section = ".requests"]
pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

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

    // 1. ИНИЦИАЛИЗАЦИЯ ПАМЯТИ
    let hhdm_offset = HHDM_REQUEST.get_response().expect("No HHDM").offset();
    kernel_core::HHDM_OFFSET.store(hhdm_offset, Ordering::Relaxed);

    let virt_offset = VirtAddr::new(hhdm_offset);
    let memory_map = MEMORY_MAP_REQUEST.get_response().expect("No Mmap").entries();

    let mut mapper = unsafe { kernel_core::memory::init_mapper(virt_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_map) };

    kernel_core::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Heap initialization failed");

    // 2. ИНИЦИАЛИЗАЦИЯ ГРАФИКИ
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

    let windows = vec![
        Window::new(50, 50, 300, 200, "Terminal", 0xFF000088), // Наш "терминал"
        Window::new(400, 100, 200, 150, "Debug Info", 0xFF444444), // Серое окно
    ];

    // 3. ФАЙЛОВАЯ СИСТЕМА
    if let Some(response) = MODULE_REQUEST.get_response() {
        if let Some(module) = response.modules().get(0) {
            let addr = module.addr();
            let size = module.size();
            unsafe {
                *kernel_core::shell::FILESYSTEM.lock() = Some(TarFileSystem::new(addr, size));
            }
            serial_println!("FS Mounted");
        }
    }

    // 4. ВВОД
    kernel_core::interrupts::init_input(); 

    // 5. ВКЛЮЧАЕМ ПРЕРЫВАНИЯ
    serial_println!("Enabling Interrupts...");
    x86_64::instructions::interrupts::enable(); 

    // --- ОТРИСОВКА ИНТЕРФЕЙСА ---
    
    // Очищаем экран ОДИН РАЗ (синий фон)
    // Используем without_interrupts даже здесь для безопасности
    x86_64::instructions::interrupts::without_interrupts(|| {
        if let Some(screen) = &mut *SCREEN.lock() {
            screen.clear(0x00000088);
        }
    });

    println!("RizOS Kernel v0.1");
    println!("Display: {}x{}", screen_width, screen_height);
    println!("\nType 'help' for commands.");
    print!("> ");

    // --- ГЛАВНЫЙ ЦИКЛ (Main Loop) ---
    loop {
        // КРИТИЧЕСКАЯ СЕКЦИЯ ОТРИСОВКИ
        x86_64::instructions::interrupts::without_interrupts(|| {
            
            // 1. СЛОЙ ФОНА
            if let Some(screen) = &mut *SCREEN.lock() {
                screen.clear(0x00224466); // Темно-синий рабочий стол
            }

            // 2. СЛОЙ ОКОН
            // Рисуем наши тестовые окна
            // (Кстати, окно терминала у нас было: Window::new(50, 50, 300, 200...))
            // Давай нарисуем его вручную или через цикл, если у тебя есть массив windows.
            // Если массива нет, нарисуем одно "Главное окно":
            if let Some(screen) = &mut *SCREEN.lock() {
                // Тень окна (смещение +5, +5, черный цвет)
                screen.fill_rect(55, 55, 800, 500, 0xFF111111);
                
                // Тело окна (Темно-синий терминал)
                screen.fill_rect(50, 50, 800, 500, 0xFF000088); 
                
                // Заголовок
                screen.fill_rect(50, 50, 800, 25, 0xFFAAAAAA); // Серый заголовок
            }

            // 3. СЛОЙ ТЕКСТА (Консоль)
            // Мы просим writer нарисовать содержимое буфера поверх окна.
            // Смещение (60, 80) — это чтобы текст был внутри синего прямоугольника
            if let Some(console) = kernel_core::writer::CONSOLE.try_lock() {
                console.draw(60, 80); 
            }

            // 4. СЛОЙ МЫШИ
            let mx = unsafe { kernel_core::interrupts::MOUSE_X };
            let my = unsafe { kernel_core::interrupts::MOUSE_Y };
            kernel_core::writer::draw_mouse(mx as usize, my as usize);

            // 5. ВЫВОД НА МОНИТОР
            if let Some(screen) = &mut *SCREEN.lock() {
                screen.present();
            }
        });

        for _ in 0..50000 { core::hint::spin_loop(); }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("PANIC: {}", info);
    hcf();
}