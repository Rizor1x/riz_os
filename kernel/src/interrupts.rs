use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use pic8259::ChainedPics;
use spin::Mutex;
use lazy_static::lazy_static;
use x86_64::instructions::port::Port;
use core::sync::atomic::{AtomicBool, Ordering};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
pub static STOP_VM: AtomicBool = AtomicBool::new(false);
pub static mut MOUSE_LEFT_PRESSED: bool = false;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
    Mouse = PIC_1_OFFSET + 12,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 { self as u8 }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::Mouse.as_u8()].set_handler_fn(mouse_interrupt_handler);
        idt
    };
}

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::Ignore));
}

pub fn init_idt() {
    IDT.load();
}

// ЕДИНАЯ ФУНКЦИЯ ИНИЦИАЛИЗАЦИИ УСТРОЙСТВ
pub fn init_input() {
    unsafe {
        // 1. Инициализируем PIC
        let mut pics = PICS.lock();
        pics.initialize();
        // Разрешаем Timer, Keyboard, Mouse (остальное маскируем, если надо, но пока 0 = всё)
        pics.write_masks(0, 0); 
    }

    // 2. Инициализируем Контроллер Клавиатуры (i8042)
    let mut command = Port::<u8>::new(0x64);
    let mut data = Port::<u8>::new(0x60);
    
    unsafe {
        // Отключаем устройства
        command.write(0xAD); // Disable Keyboard
        command.write(0xA7); // Disable Mouse
        
        // Читаем конфиг
        command.write(0x20);
        while command.read() & 1 == 0 {} // Wait for output
        let mut config = data.read();
        
        // Включаем IRQ1 (Клава) и IRQ12 (Мышь)
        config |= 0x01; // Keyboard IRQ
        config |= 0x02; // Mouse IRQ
        config |= 0x40; // Translation
        
        // Пишем конфиг обратно
        command.write(0x60);
        while command.read() & 2 != 0 {} // Wait for input buffer
        data.write(config);
        
        // Включаем устройства обратно
        command.write(0xAE); // Enable Keyboard
        command.write(0xA8); // Enable Mouse
        
        // Настройка мыши
        command.write(0xD4);
        while command.read() & 2 != 0 {}
        data.write(0xF4); // Enable Scanning
        while command.read() & 1 == 0 {}
        data.read(); // ACK
    }
}

pub fn handle_keyboard_raw(scancode: u8) {
    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    if character == '\x1b' {
                        STOP_VM.store(true, Ordering::Relaxed);
                        crate::print!("^ESC");
                    } else {
                        crate::shell::handle_keystroke(character);
                    }
                },
                DecodedKey::RawKey(_) => {},
            }
        }
    }
}

pub fn handle_mouse_raw(packet: u8) {
    unsafe {
        match MOUSE_CYCLE {
            0 => { if (packet & 0x08) != 0 { MOUSE_PACKET[0] = packet; MOUSE_CYCLE = 1; } }
            1 => { MOUSE_PACKET[1] = packet; MOUSE_CYCLE = 2; }
            2 => {
                MOUSE_PACKET[2] = packet;
                MOUSE_CYCLE = 0;
                let _header = MOUSE_PACKET[0];

                MOUSE_LEFT_PRESSED = (_header & 0x01) != 0;
                
                let mut dx = MOUSE_PACKET[1] as i8 as i32;
                let mut dy = MOUSE_PACKET[2] as i8 as i32;
                let speed = 2;
                dx *= speed; dy *= speed;
                
                MOUSE_X += dx;
                MOUSE_Y -= dy;
                MOUSE_X = MOUSE_X.clamp(0, 1275);
                MOUSE_Y = MOUSE_Y.clamp(0, 795);
            }
            _ => MOUSE_CYCLE = 0,
        }
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()); }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    handle_keyboard_raw(scancode); // Вызываем логику
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8()); }
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let packet = unsafe { port.read() };
    handle_mouse_raw(packet); // Вызываем логику
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Mouse.as_u8()); }
}

// ... (остальные обработчики без изменений) ...
// (Не забудь статические переменные MOUSE_CYCLE и т.д. тоже оставить)
static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_PACKET: [u8; 3] = [0; 3];
pub static mut MOUSE_X: i32 = 500;
pub static mut MOUSE_Y: i32 = 300;