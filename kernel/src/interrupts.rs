use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{serial_println}; 
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin::Mutex;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // Исключения
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(0);
        }
        
        // --- ИСПРАВЛЕНИЕ: Используем as_u8() вместо as_usize() ---
        
        // Таймер [32]
        idt[InterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_interrupt_handler);
            
        // Клавиатура [33]
        idt[InterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_interrupt_handler);
        
        idt
    };
}

pub fn init_idt() {
    IDT.load();
    unsafe { 
        let mut pics = PICS.lock();
        pics.initialize();
        // ВАЖНО: Явно разрешаем все прерывания (снимаем маски)
        // 0 = разрешено, 1 = запрещено. Пишем 0 во все каналы.
        pics.write_masks(0, 0); 
    }
}

// --- Обработчики ---

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    // ВМЕСТО ЛОГИКИ ДЕКОДИРОВАНИЯ МЫ ПРОСТО КИДАЕМ БАЙТ В ОЧЕРЕДЬ
    // Нам нужно добраться до функции add_scancode.
    // Т.к. она pub(crate), она видна внутри крейта.
    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}


pub fn enable_keyboard() {
    use x86_64::instructions::port::Port;
    
    let mut command_port = Port::<u8>::new(0x64);
    let mut data_port = Port::<u8>::new(0x60);
    
    unsafe {
        // 1. Ждем, пока контроллер будет готов принять команду
        // (бит 1 в статусном регистре должен быть 0)
        while command_port.read() & 0x02 != 0 {}
        
        // 2. Посылаем команду 0xAE (Включить порт клавиатуры)
        command_port.write(0xAE);
        
        // 3. Ждем готовности
        while command_port.read() & 0x02 != 0 {}
        
        // 4. Посылаем команду 0x20 (Прочитать конфигурацию)
        command_port.write(0x20);
        
        // 5. Ждем данные (бит 0 должен стать 1)
        while command_port.read() & 0x01 == 0 {}
        let mut status = data_port.read();
        
        // 6. Включаем прерывание клавиатуры (Бит 0 = 1) и трансляцию (Бит 6 = 1)
        status |= 0x01; // Enable IRQ 1
        status |= 0x40; // Enable Translation (scancode set 1)
        
        // 7. Записываем конфигурацию обратно
        while command_port.read() & 0x02 != 0 {}
        command_port.write(0x60); // Команда "Записать конфигурацию"
        
        while command_port.read() & 0x02 != 0 {}
        data_port.write(status);
        
        // 8. На всякий случай сбрасываем саму клавиатуру
        data_port.write(0xF4); // Enable scanning
    }
}