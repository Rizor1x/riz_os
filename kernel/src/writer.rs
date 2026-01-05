use font8x8::{UnicodeFonts, BASIC_FONTS};
use spin::Mutex;
use core::fmt;

pub static WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);

// Размер курсора
const MOUSE_SIZE: usize = 10; 

pub struct FrameBufferWriter {
    buffer: *mut u8,
    pitch: u64,
    width: u64,
    height: u64,
    bpp: u16,
    x_pos: u64,
    y_pos: u64,
    
    // Для мыши: храним позицию и фон под курсором
    mouse_x: usize,
    mouse_y: usize,
    mouse_bg_buffer: [u32; MOUSE_SIZE * MOUSE_SIZE], // Буфер для сохранения фона
    mouse_active: bool,
}

unsafe impl Send for FrameBufferWriter {}

impl FrameBufferWriter {
    pub fn new(buffer: *mut u8, pitch: u64, width: u64, height: u64, bpp: u16) -> Self {
        Self {
            buffer, pitch, width, height, bpp,
            x_pos: 10, y_pos: 10,
            mouse_x: width as usize / 2,
            mouse_y: height as usize / 2,
            mouse_bg_buffer: [0; MOUSE_SIZE * MOUSE_SIZE],
            mouse_active: false,
        }
    }

    // --- ФУНКЦИИ РИСОВАНИЯ ---

    pub fn read_pixel(&self, x: u64, y: u64) -> u32 {
        if x >= self.width || y >= self.height { return 0; }
        let offset = y * self.pitch + x * ((self.bpp as u64) / 8);
        unsafe {
            let ptr = self.buffer.add(offset as usize) as *const u32;
            *ptr
        }
    }

    pub fn draw_pixel(&mut self, x: u64, y: u64, color: u32) {
        if x >= self.width || y >= self.height { return; }
        let offset = y * self.pitch + x * ((self.bpp as u64) / 8);
        unsafe {
            let ptr = self.buffer.add(offset as usize) as *mut u32;
            // Используем volatile write, чтобы компилятор не шалил
            core::ptr::write_volatile(ptr, color);
        }
    }

    // --- ЛОГИКА МЫШИ ---

    pub fn update_mouse(&mut self, new_x: usize, new_y: usize) {
        // 1. Если мышь уже была нарисована, стираем её (восстанавливаем фон)
        if self.mouse_active {
            for dy in 0..MOUSE_SIZE {
                for dx in 0..MOUSE_SIZE {
                    let bg_color = self.mouse_bg_buffer[dy * MOUSE_SIZE + dx];
                    self.draw_pixel((self.mouse_x + dx) as u64, (self.mouse_y + dy) as u64, bg_color);
                }
            }
        }

        // 2. Обновляем координаты
        self.mouse_x = new_x;
        self.mouse_y = new_y;

        // 3. Сохраняем фон под НОВЫМ местом
        for dy in 0..MOUSE_SIZE {
            for dx in 0..MOUSE_SIZE {
                let color = self.read_pixel((new_x + dx) as u64, (new_y + dy) as u64);
                self.mouse_bg_buffer[dy * MOUSE_SIZE + dx] = color;
            }
        }

        // 4. Рисуем курсор (зеленый квадрат с белой окантовкой)
        for dy in 0..MOUSE_SIZE {
            for dx in 0..MOUSE_SIZE {
                // Простейшая форма: рамка
                if dx == 0 || dx == MOUSE_SIZE - 1 || dy == 0 || dy == MOUSE_SIZE - 1 {
                    self.draw_pixel((new_x + dx) as u64, (new_y + dy) as u64, 0xFFFFFFFF); // Белый
                } else {
                    self.draw_pixel((new_x + dx) as u64, (new_y + dy) as u64, 0xFF00FF00); // Зеленый
                }
            }
        }
        
        self.mouse_active = true;
    }

    // --- ТЕКСТОВЫЕ ФУНКЦИИ ---

    pub fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.new_line(),
            '\x08' => { // Backspace
                if self.x_pos >= 18 {
                    self.x_pos -= 8;
                    self.draw_rect(self.x_pos, self.y_pos, 8, 12, 0x00000088); // Закрашиваем синим
                }
            },
            _ => {
                if self.x_pos + 8 >= self.width { self.new_line(); }
                self.draw_char_pixels(c);
                self.x_pos += 8;
            }
        }
    }

    fn draw_rect(&mut self, x: u64, y: u64, w: u64, h: u64, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.draw_pixel(x + dx, y + dy, color);
            }
        }
    }

    fn new_line(&mut self) {
        self.x_pos = 10;
        self.y_pos += 14;
        if self.y_pos + 14 >= self.height { self.y_pos = 10; } // Сброс
    }

    fn draw_char_pixels(&mut self, c: char) {
        let bitmap = match BASIC_FONTS.get(c) {
            Some(g) => g,
            None => return,
        };
        for (y, row) in bitmap.iter().enumerate() {
            for x in 0..8 {
                if *row & (1 << x) != 0 {
                    self.draw_pixel(self.x_pos + x as u64, self.y_pos + y as u64, 0xFFFFFFFF);
                }
            }
        }
    }
}

// Реализация Write для макросов
impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() { self.write_char(c); }
        Ok(())
    }
}

// --- МАКРОСЫ И ГЛОБАЛЬНЫЕ ФУНКЦИИ ---

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    
    interrupts::without_interrupts(|| {
        if let Some(writer) = &mut *WRITER.lock() {
            writer.write_fmt(args).unwrap();
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

// Функция для обновления мыши из main.rs
pub fn update_mouse_cursor(x: usize, y: usize) {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        if let Some(writer) = &mut *WRITER.lock() {
            writer.update_mouse(x, y);
        }
    });
}