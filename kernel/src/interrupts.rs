use pc_keyboard::Keyboard;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{serial_println, print, println}; 
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin::Mutex;
use alloc::string::String; // Нам нужна строка

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

// Буфер для накопления команды пользователя
lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<pc_keyboard::layouts::Us104Key, pc_keyboard::ScancodeSet1>> =
        Mutex::new(Keyboard::new(pc_keyboard::ScancodeSet1::new(), pc_keyboard::layouts::Us104Key, pc_keyboard::HandleControl::Ignore));
        
    // Строка, куда мы собираем буквы
    static ref COMMAND_BUFFER: Mutex<String> = Mutex::new(String::new());
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    // ... (импорты портов и кейборда остаются, lazy_static мы вынесли выше) ...

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    
    let mut keyboard = KEYBOARD.lock();
    let mut buffer = COMMAND_BUFFER.lock(); // Блокируем буфер

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                pc_keyboard::DecodedKey::Unicode(character) => {
                    match character {
                        '\n' => {
                            // ЕСЛИ НАЖАЛИ ENTER:
                            println!(); // Перенос строки на экране
                            
                            // Выполняем команду
                            execute_command(&buffer);
                            
                            // Очищаем буфер для следующей команды
                            buffer.clear();
                            print!("> "); // Рисуем приглашение к вводу
                        },
                        '\x08' => {
                            // ЕСЛИ НАЖАЛИ BACKSPACE:
                            if !buffer.is_empty() {
                                // Удаляем последний символ из строки
                                buffer.pop(); 
                                // Печатаем Backspace на экран (writer.rs сам сотрет букву)
                                print!("{}", character); 
                            }
                        },
                        _ => {
                            // ОБЫЧНАЯ БУКВА:
                            buffer.push(character); // Запоминаем
                            print!("{}", character); // Рисуем
                        }
                    }
                },
                pc_keyboard::DecodedKey::RawKey(_) => {}, // Игнорируем спецклавиши
            }
        }
    }

    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

// Функция выполнения команд
fn execute_command(command: &str) {
    match command.trim() { // trim убирает пробелы
        "help" => {
            println!("RizOS Help Menu:");
            println!("  help  - Show this message");
            println!("  ver   - Show version");
            println!("  clear - Clear screen");
        },
        "ver" => {
            println!("RizOS v0.1.0 (2026)");
        },
        "clear" => {
             // Чтобы это сработало, нужно сделать writer::WRITER public и добавить метод clear
             // Пока просто напечатаем много пустоты
             println!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n"); 
        },
        "" => {}, // Пустая команда - ничего не делаем
        _ => {
            println!("Unknown command: '{}'", command);
        }
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