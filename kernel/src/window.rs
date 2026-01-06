use crate::graphics::SCREEN;
use alloc::vec::Vec;
use alloc::vec;

pub struct Window {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub title: &'static str,
    pub color: u32,
    dragging: bool, // Тянем ли мы окно?
}

impl Window {
    pub fn new(x: usize, y: usize, w: usize, h: usize, title: &'static str, color: u32) -> Self {
        Self { x, y, width: w, height: h, title, color, dragging: false }
    }

    pub fn draw(&self) {
        if let Some(screen) = &mut *SCREEN.lock() {
            // 1. Тело окна
            screen.fill_rect(self.x, self.y, self.width, self.height, self.color);
            
            // 2. Заголовок (Title Bar) - Темнее
            screen.fill_rect(self.x, self.y, self.width, 25, 0xFF333333);
            
            // 3. Рамка (Border) - Светлая
            // (Упрощенно: просто белая линия сверху)
            screen.fill_rect(self.x, self.y, self.width, 2, 0xFFFFFFFF);
            
            // Тут можно добавить отрисовку текста заголовка, 
            // но для этого нужно прокинуть writer или font8x8 сюда.
        }
    }
}