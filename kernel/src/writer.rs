use font8x8::{UnicodeFonts, BASIC_FONTS};
use spin::Mutex;
use core::fmt;

// Глобальный писатель. Изначально пустой (None), инициализируем при старте.
pub static WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);

pub struct FrameBufferWriter {
    buffer: *mut u8,
    pitch: u64,
    width: u64,
    height: u64,
    bpp: u16,
    x_pos: u64,
    y_pos: u64,
}

// Разрешаем передавать указатель между потоками (безопасно, т.к. есть Mutex)
unsafe impl Send for FrameBufferWriter {}

impl FrameBufferWriter {
    pub fn new(buffer: *mut u8, pitch: u64, width: u64, height: u64, bpp: u16) -> Self {
        Self {
            buffer,
            pitch,
            width,
            height,
            bpp,
            x_pos: 10, // Отступ слева
            y_pos: 10, // Отступ сверху
        }
    }

    pub fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.new_line(),
            // Обработка Backspace (удаление символа)
            '\x08' => {
                if self.x_pos >= 18 { // 10 отступ + 8 ширина буквы
                    self.x_pos -= 8;
                    // Рисуем черный (или синий) квадрат поверх буквы
                    self.draw_cursor_block(0x00000088); 
                }
            },
            _ => {
                if self.x_pos + 8 >= self.width {
                    self.new_line();
                }
                self.draw_char_pixels(c);
                self.x_pos += 8;
            }
        }
    }

    fn new_line(&mut self) {
        self.x_pos = 10;
        self.y_pos += 12; // 8 высота + 4 отступ
        if self.y_pos + 8 >= self.height {
            self.y_pos = 10; // Пока просто сбрасываем вверх
            self.clear_screen();
        }
    }

    fn clear_screen(&mut self) {
         for y in 0..self.height {
            for x in 0..self.width {
                self.draw_pixel(x, y, 0x00000088); // Синий цвет
            }
        }
    }

    fn draw_cursor_block(&self, color: u32) {
        for y in 0..8 {
            for x in 0..8 {
                self.draw_pixel(self.x_pos + x, self.y_pos + y, color);
            }
        }
    }

    fn draw_char_pixels(&self, c: char) {
        let bitmap = match BASIC_FONTS.get(c) {
            Some(glyph) => glyph,
            None => return, // Если символа нет в шрифте
        };

        for (y, row) in bitmap.iter().enumerate() {
            for x in 0..8 {
                match *row & (1 << x) {
                    0 => {}, // Фон
                    _ => self.draw_pixel(self.x_pos + x as u64, self.y_pos + y as u64, 0xFFFFFFFF), // Белый текст
                }
            }
        }
    }

    fn draw_pixel(&self, x: u64, y: u64, color: u32) {
        let pixel_offset = y * self.pitch + x * ((self.bpp as u64) / 8);
        unsafe {
            let ptr = self.buffer.add(pixel_offset as usize) as *mut u32;
            *ptr = color;
        }
    }
}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

// --- МАКРОСЫ ---

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // Блокируем writer и пишем, если он инициализирован
    if let Some(writer) = &mut *WRITER.lock() {
        writer.write_fmt(args).unwrap();
    }
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