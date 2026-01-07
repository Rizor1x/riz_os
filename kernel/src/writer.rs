use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use crate::graphics::Screen; // Импорт типа Screen

const COLS: usize = 100;
const ROWS: usize = 40;

lazy_static! {
    pub static ref CONSOLE: Mutex<Console> = Mutex::new(Console::new());
}

pub struct Console {
    buffer: [[char; COLS]; ROWS],
    x: usize,
    y: usize,
}

impl Console {
    pub fn new() -> Self {
        Self { buffer: [[' '; COLS]; ROWS], x: 0, y: 0 }
    }

    pub fn write_char(&mut self, c: char) {
        // write_char используется из прерываний или макросов, 
        // поэтому тут lock() нужен, так как мы не в главном цикле отрисовки.
        // Но чтобы не было конфликтов, лучше писать только в буфер текста,
        // а отрисовку делать отдельно.
        match c {
            '\n' => self.newline(),
            '\x08' => {
                if self.x > 0 { self.x -= 1; self.buffer[self.y][self.x] = ' '; }
                else if self.y > 0 { self.y -= 1; self.x = COLS - 1; self.buffer[self.y][self.x] = ' '; }
            },
            _ => {
                if self.x >= COLS { self.newline(); }
                self.buffer[self.y][self.x] = c;
                self.x += 1;
            }
        }
    }

    fn newline(&mut self) {
        self.x = 0;
        if self.y < ROWS - 1 { self.y += 1; }
        else {
            for row in 1..ROWS { self.buffer[row - 1] = self.buffer[row]; }
            self.buffer[ROWS - 1] = [' '; COLS];
        }
    }

    // ИСПРАВЛЕНИЕ: Принимаем screen как аргумент!
    pub fn draw(&self, screen: &mut Screen, offset_x: usize, offset_y: usize) {
        // Убрали lock() здесь!
        for row in 0..ROWS {
            for col in 0..COLS {
                let c = self.buffer[row][col];
                if c != ' ' {
                    draw_char_on_screen(screen, c, offset_x + col * 8, offset_y + row * 14);
                }
            }
        }
        // Курсор
        screen.fill_rect(offset_x + self.x * 8, offset_y + self.y * 14, 8, 14, 0xFFFFFFFF);
    }
}

fn draw_char_on_screen(screen: &mut Screen, c: char, x: usize, y: usize) {
    if let Some(glyph) = BASIC_FONTS.get(c) {
        for (dy, row) in glyph.iter().enumerate() {
            for dx in 0..8 {
                if *row & (1 << dx) != 0 {
                    screen.put_pixel(x + dx, y + dy, 0xFFFFFFFF);
                }
            }
        }
    }
}

// ИСПРАВЛЕНИЕ: Принимаем screen как аргумент!
pub fn draw_mouse(screen: &mut Screen, x: usize, y: usize) {
    // Убрали lock() здесь!
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

// Заглушка для совместимости со старым кодом (если где-то остался вызов)
pub fn update_mouse_cursor(_x: usize, _y: usize) {}

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

#[macro_export] macro_rules! print { ($($arg:tt)*) => ($crate::writer::_print(format_args!($($arg)*))); }
#[macro_export] macro_rules! println { () => ($crate::print!("\n")); ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*))); }