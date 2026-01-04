use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::serial_println;
use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // 1. Назначаем обработчик для Breakpoint (тестовое прерывание)
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        
        // 2. Назначаем обработчик для Double Fault (критическая ошибка)
        // Позже нам придется добавить сюда переключение стека, но пока так
        idt.double_fault.set_handler_fn(double_fault_handler);
        
        idt
    };
}

pub fn init_idt() {
    // Загружаем IDT в процессор
    IDT.load();
}

// --- ОБРАБОТЧИКИ ---

// extern "x86-interrupt" - это специальное соглашение о вызовах.
// Процессор сохраняет состояние совсем не так, как при вызове обычной функции.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

// Double Fault возникает, когда процессор не смог вызвать другой обработчик.
// Если мы не обработаем Double Fault, случится Triple Fault (ребут).
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, 
    _error_code: u64
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}