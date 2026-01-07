use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use pic8259::ChainedPics;
use spin::Mutex;
use lazy_static::lazy_static;
use x86_64::instructions::port::Port;
use core::sync::atomic::{AtomicBool, Ordering};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub const TIMER_INTERRUPT_ID: u8 = PIC_1_OFFSET;
pub const KEYBOARD_INTERRUPT_ID: u8 = PIC_1_OFFSET + 1;
pub const MOUSE_INTERRUPT_ID: u8 = PIC_1_OFFSET + 12;

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
pub static STOP_VM: AtomicBool = AtomicBool::new(false);
pub static VM_ACTIVE: AtomicBool = AtomicBool::new(false);

// Глобальные переменные мыши
pub static mut MOUSE_X: i32 = 500;
pub static mut MOUSE_Y: i32 = 300;
pub static mut MOUSE_LEFT_PRESSED: bool = false;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX - 1);
        }
        idt[TIMER_INTERRUPT_ID].set_handler_fn(timer_interrupt_handler);
        idt[KEYBOARD_INTERRUPT_ID].set_handler_fn(keyboard_interrupt_handler);
        idt[MOUSE_INTERRUPT_ID].set_handler_fn(mouse_interrupt_handler);
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

pub fn init_input() {
    unsafe {
        let mut pics = PICS.lock();
        pics.initialize();
        pics.write_masks(0xF8, 0xEF); 
    }
    let mut command = Port::<u8>::new(0x64);
    let mut data = Port::<u8>::new(0x60);
    unsafe {
        command.write(0xAD); command.write(0xA7);
        command.write(0x20); while command.read() & 1 == 0 {}
        let mut config = data.read();
        config |= 0x01; config |= 0x02; config |= 0x40;
        command.write(0x60); while command.read() & 2 != 0 {}
        data.write(config);
        command.write(0xAE); command.write(0xA8);
        command.write(0xD4); while command.read() & 2 != 0 {}
        data.write(0xF4); while command.read() & 1 == 0 {}
        data.read();
    }
}

// --- ПУБЛИЧНАЯ ЛОГИКА (ВЫЗЫВАЕТСЯ ОТОВСЮДУ) ---

pub fn handle_keyboard_raw(scancode: u8) {
    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    if character == '\x1b' {
                        STOP_VM.store(true, Ordering::Relaxed);
                    } else {
                        crate::shell::handle_keystroke(character);
                    }
                },
                DecodedKey::RawKey(_) => {},
            }
        }
    }
}

static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_PACKET: [u8; 3] = [0; 3];

pub fn handle_mouse_raw(packet: u8) {
    unsafe {
        match MOUSE_CYCLE {
            0 => { if (packet & 0x08) != 0 { MOUSE_PACKET[0] = packet; MOUSE_CYCLE = 1; } }
            1 => { MOUSE_PACKET[1] = packet; MOUSE_CYCLE = 2; }
            2 => {
                MOUSE_PACKET[2] = packet;
                MOUSE_CYCLE = 0;
                let header = MOUSE_PACKET[0];
                MOUSE_LEFT_PRESSED = (header & 0x01) != 0;
                let mut dx = MOUSE_PACKET[1] as i8 as i32;
                let mut dy = MOUSE_PACKET[2] as i8 as i32;
                dx *= 2; dy *= 2;
                MOUSE_X += dx;
                MOUSE_Y -= dy;
                MOUSE_X = MOUSE_X.clamp(0, 2000); // Примерные границы
                MOUSE_Y = MOUSE_Y.clamp(0, 2000);
            }
            _ => MOUSE_CYCLE = 0,
        }
    }
}

// --- ОБРАБОТЧИКИ ПРЕРЫВАНИЙ (ТОЛЬКО ОБЕРТКИ) ---

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe { PICS.lock().notify_end_of_interrupt(TIMER_INTERRUPT_ID as u8); }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    
    // Вызываем общую логику
    handle_keyboard_raw(scancode);
    
    unsafe { PICS.lock().notify_end_of_interrupt(KEYBOARD_INTERRUPT_ID as u8); }
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let packet = unsafe { port.read() };
    
    // Вызываем общую логику
    handle_mouse_raw(packet);
    
    unsafe { PICS.lock().notify_end_of_interrupt(MOUSE_INTERRUPT_ID as u8); }
}