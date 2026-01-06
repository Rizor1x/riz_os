use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;
use lazy_static::lazy_static;

// Глобальный экран
lazy_static! {
    pub static ref SCREEN: Mutex<Option<Screen>> = Mutex::new(None);
}

pub struct Screen {
    framebuffer_addr: *mut u32, // Реальная видеопамять
    backbuffer: Vec<u32>,       // Наш буфер в RAM (рисуем сюда)
    width: usize,
    height: usize,
    pitch: usize,               // Длина строки в байтах (для framebuffer)
}

// Разрешаем передавать между потоками
unsafe impl Send for Screen {}

impl Screen {
    pub fn new(addr: *mut u8, width: usize, height: usize, pitch: usize) -> Self {
        // Выделяем память под буфер (width * height пикселей)
        let buffer_size = width * height;
        let backbuffer = vec![0xFF000088; buffer_size]; // Заливаем синим сразу

        Self {
            framebuffer_addr: addr as *mut u32,
            backbuffer,
            width,
            height,
            pitch, // pitch в байтах, но мы пишем u32 (4 байта)
        }
    }

    // Очистка буфера (заливка цветом)
    pub fn clear(&mut self, color: u32) {
        for pixel in self.backbuffer.iter_mut() {
            *pixel = color;
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.backbuffer[y * self.width + x]
        } else {
            0 // Черный, если вышли за границы
        }
    }

    // Рисование пикселя в буфер (безопасно и быстро)
    pub fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.backbuffer[y * self.width + x] = color;
        }
    }

    // Рисование прямоугольника
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }

    // САМОЕ ВАЖНОЕ: Копирование буфера на экран (Swap Buffers)
    pub fn present(&mut self) {
        for y in 0..self.height {
            // Вычисляем смещения
            let buffer_start = y * self.width;
            let fb_offset = y * (self.pitch / 4); // pitch в байтах / 4 = pitch в пикселях

            unsafe {
                // Используем `copy_nonoverlapping` для максимальной скорости (как memcpy в C)
                // Копируем целую строку пикселей за раз!
                core::ptr::copy_nonoverlapping(
                    self.backbuffer.as_ptr().add(buffer_start),
                    self.framebuffer_addr.add(fb_offset),
                    self.width
                );
            }
        }
    }
    
    // Геттеры
    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}