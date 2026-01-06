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



    // Состояние окна
    let mut win_x = 50;
    let mut win_y = 50;
    let win_w = 800;
    let win_h = 500;

    // Флаг: тянем ли мы окно сейчас?
    let mut is_dragging = false;
    // Запоминаем, за какую часть заголовка схватили (смещение)
    let mut drag_offset_x = 0;
    let mut drag_offset_y = 0;

    // --- ГЛАВНЫЙ ЦИКЛ (Main Loop) ---
    loop {
        // КРИТИЧЕСКАЯ СЕКЦИЯ ОТРИСОВКИ
        x86_64::instructions::interrupts::without_interrupts(|| {
            
            let mx = unsafe { kernel_core::interrupts::MOUSE_X as usize };
            let my = unsafe { kernel_core::interrupts::MOUSE_Y as usize };
            let lmb = unsafe { kernel_core::interrupts::MOUSE_LEFT_PRESSED };

            // ЛОГИКА ПЕРЕТАСКИВАНИЯ
            if lmb {
                if is_dragging {
                    // Уже тянем -> обновляем позицию окна
                    // Новая позиция = Текущая мышь - Смещение захвата
                    // (Используем saturating_sub чтобы не улететь в минус)
                    win_x = mx.saturating_sub(drag_offset_x);
                    win_y = my.saturating_sub(drag_offset_y);
                } else {
                    // Кликнули только что. Проверяем, попали ли в ЗАГОЛОВОК окна?
                    // Заголовок это область от (win_x, win_y) до (win_x + w, win_y + 30)
                    if mx >= win_x && mx <= win_x + win_w && 
                        my >= win_y && my <= win_y + 30 {
                        is_dragging = true;
                        drag_offset_x = mx - win_x;
                        drag_offset_y = my - win_y;
                    }
                }
            } else {
                // Кнопка отпущена -> перестаем тянуть
                is_dragging = false;
            }

            // 1. Очистка фона
            if let Some(screen) = &mut *SCREEN.lock() {
                screen.clear(0x00224466);
                
                // Тень
                screen.fill_rect(win_x + 10, win_y + 10, win_w, win_h, 0xFF111111);
                // Окно
                screen.fill_rect(win_x, win_y, win_w, win_h, 0xFF000088);
                // Заголовок (меняем цвет, если тащим)
                let title_color = if is_dragging { 0xFF5555AA } else { 0xFFAAAAAA };
                screen.fill_rect(win_x, win_y, win_w, 30, title_color);
            }

            // 3. Текст
            // ВАЖНО: Мы должны сказать консоли рисовать текст по НОВЫМ координатам окна!
            // Смещение текста внутри окна: +10 по X, +35 по Y (под заголовком)
            if let Some(console) = kernel_core::writer::CONSOLE.try_lock() {
                console.draw(win_x + 10, win_y + 35); 
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