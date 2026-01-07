use alloc::vec::Vec;
use alloc::vec; // <--- ВАЖНО: Импорт макроса
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SCREEN: Mutex<Option<Screen>> = Mutex::new(None);
}

pub struct Screen {
    framebuffer_addr: *mut u32,
    backbuffer: Vec<u32>,
    width: usize,
    height: usize,
    pitch: usize,
}

unsafe impl Send for Screen {}

impl Screen {
    pub fn new(addr: *mut u8, width: usize, height: usize, pitch: usize) -> Self {
        let buffer_size = width * height;
        let backbuffer = vec![0xFF000088; buffer_size]; // Синий фон

        Self {
            framebuffer_addr: addr as *mut u32,
            backbuffer,
            width,
            height,
            pitch,
        }
    }

    pub fn clear(&mut self, color: u32) {
        for pixel in self.backbuffer.iter_mut() {
            *pixel = color;
        }
    }

    // Получить цвет пикселя (нужно для прозрачности мыши, если захотим)
    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.backbuffer[y * self.width + x]
        } else {
            0
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.backbuffer[y * self.width + x] = color;
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }

    pub fn present(&mut self) {
        for y in 0..self.height {
            let buffer_start = y * self.width;
            let fb_offset = y * (self.pitch / 4);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    self.backbuffer.as_ptr().add(buffer_start),
                    self.framebuffer_addr.add(fb_offset),
                    self.width
                );
            }
        }
    }
    
    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}