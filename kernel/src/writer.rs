use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use crate::graphics::SCREEN;

// Размеры консоли (в символах)
const COLS: usize = 100;
const ROWS: usize = 40;

// Глобальная консоль
lazy_static! {
    pub static ref CONSOLE: Mutex<Console> = Mutex::new(Console::new());
}

pub struct Console {
    buffer: [[char; COLS]; ROWS], // Память текста
    x: usize,
    y: usize,
}

impl Console {
    pub fn new() -> Self {
        Self {
            buffer: [[' '; COLS]; ROWS], // Заполняем пробелами
            x: 0,
            y: 0,
        }
    }

    pub fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\x08' => { // Backspace
                if self.x > 0 {
                    self.x -= 1;
                    self.buffer[self.y][self.x] = ' ';
                } else if self.y > 0 { // Подняться на строку вверх
                     self.y -= 1;
                     self.x = COLS - 1;
                     self.buffer[self.y][self.x] = ' ';
                }
            },
            _ => {
                if self.x >= COLS {
                    self.newline();
                }
                self.buffer[self.y][self.x] = c;
                self.x += 1;
            }
        }
    }

    fn newline(&mut self) {
        self.x = 0;
        if self.y < ROWS - 1 {
            self.y += 1;
        } else {
            // Скроллинг: сдвигаем все строки вверх
            for row in 1..ROWS {
                self.buffer[row - 1] = self.buffer[row];
            }
            // Очищаем последнюю строку
            self.buffer[ROWS - 1] = [' '; COLS];
        }
    }

    // Эта функция вызывается каждый кадр в main.rs
    // Она рисует текст поверх окон
    pub fn draw(&self, offset_x: usize, offset_y: usize) {
        if let Some(screen) = &mut *SCREEN.lock() {
            for row in 0..ROWS {
                for col in 0..COLS {
                    let c = self.buffer[row][col];
                    if c != ' ' {
                        // Рисуем символ
                        // x = смещение окна + колонка * 8 пикселей
                        // y = смещение окна + строка * 14 пикселей
                        draw_char_on_screen(screen, c, offset_x + col * 8, offset_y + row * 14);
                    }
                }
            }
            
            // Рисуем курсор (мигающий квадрат)
            // (Простой белый квадрат после последней буквы)
            let cursor_x = offset_x + self.x * 8;
            let cursor_y = offset_y + self.y * 14;
            // Рисуем прямоугольник курсора (8x14)
            screen.fill_rect(cursor_x, cursor_y, 8, 14, 0xFFFFFFFF);
        }
    }
}

// Хелпер для рисования одной буквы
fn draw_char_on_screen(screen: &mut crate::graphics::Screen, c: char, x: usize, y: usize) {
    if let Some(glyph) = BASIC_FONTS.get(c) {
        for (dy, row) in glyph.iter().enumerate() {
            for dx in 0..8 {
                if *row & (1 << dx) != 0 {
                    screen.put_pixel(x + dx, y + dy, 0xFFFFFFFF); // Белый текст
                }
            }
        }
    }
}

// --- МЫШЬ (Просто обновляет координаты) ---
// Рисует теперь main.rs, writer только хранит координаты (хотя лучше бы это тоже вынести, но оставим)
pub static mut OLD_MOUSE_X: usize = 0;
pub static mut OLD_MOUSE_Y: usize = 0;

pub fn update_mouse_cursor(_x: usize, _y: usize) {
    // Эта функция теперь пустая или может просто обновлять глобальные переменные,
    // если мы хотим хранить их тут. Но main.rs берет их из interrupts.
    // Оставим пустой для совместимости, чтобы код компилировался.
}

// Функция рисования мыши для main.rs
pub fn draw_mouse(x: usize, y: usize) {
    if let Some(screen) = &mut *SCREEN.lock() {
        // Зеленый курсор 10x10 с рамкой
        for dy in 0..10 {
            for dx in 0..10 {
                if dx == 0 || dx == 9 || dy == 0 || dy == 9 {
                    screen.put_pixel(x + dx, y + dy, 0xFFFFFFFF);
                } else {
                    screen.put_pixel(x + dx, y + dy, 0xFF00FF00);
                }
            }
        }
    }
}

// --- МАКРОСЫ ---
impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() { self.write_char(c); }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    
    interrupts::without_interrupts(|| {
        if let Some(mut console) = CONSOLE.try_lock() {
            console.write_fmt(args).unwrap();
        }
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::writer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}