use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

// ИСПОЛЬЗУЕМ МАКРОС ПРАВИЛЬНО:
lazy_static! {
    // Создаем защищенный мьютексом порт
    pub static ref SERIAL1: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });
}

pub fn init() {
    // В новой версии uart_16550 init() безопасен, unsafe не нужен
    SERIAL1.lock().init();
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // Отключаем прерывания на время печати, чтобы не было дедлоков (на будущее)
    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
    });
}

// --- МАКРОСЫ ---

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\n"), $($arg)*));
}