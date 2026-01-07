use alloc::vec::Vec;
use alloc::string::String;
use crate::graphics::Screen;

pub struct Window {
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub title: String,
    pub color: u32,
}

impl Window {
    pub fn new(id: usize, x: usize, y: usize, w: usize, h: usize, title: &str, color: u32) -> Self {
        Self {
            id, x, y, width: w, height: h,
            title: String::from(title), color,
        }
    }

    pub fn draw(&self, screen: &mut Screen) {
        screen.fill_rect(self.x + 8, self.y + 8, self.width, self.height, 0xFF111111); // Тень
        screen.fill_rect(self.x, self.y, self.width, self.height, self.color); // Окно
        screen.fill_rect(self.x, self.y, self.width, 30, 0xFF333333); // Заголовок
        
        // Кнопка закрытия (Красная)
        screen.fill_rect(self.x + self.width - 25, self.y + 5, 20, 20, 0xFFCC0000); 
    }
}

pub struct WindowManager {
    windows: Vec<Window>,
    dragging_idx: Option<usize>,
    drag_offset_x: usize,
    drag_offset_y: usize,
}

impl WindowManager {
    pub fn new() -> Self {
        Self { windows: Vec::new(), dragging_idx: None, drag_offset_x: 0, drag_offset_y: 0 }
    }

    pub fn add(&mut self, window: Window) {
        self.windows.push(window);
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Window> {
        self.windows.iter()
    }

    pub fn update_mouse(&mut self, mx: usize, my: usize, left_btn: bool) {
        if left_btn {
            if let Some(idx) = self.dragging_idx {
                // Если уже тащим
                let win = &mut self.windows[idx];
                win.x = mx.saturating_sub(self.drag_offset_x);
                win.y = my.saturating_sub(self.drag_offset_y);
            } else {
                // Только нажали. Ищем окно (сверху вниз)
                let mut found_drag = None;
                let mut found_close = None;

                for (i, win) in self.windows.iter().enumerate().rev() {
                    // 1. Проверяем Кнопку Закрытия (X)
                    // Координаты: x + width - 25, y + 5 (20x20)
                    let btn_x = win.x + win.width - 25;
                    let btn_y = win.y + 5;
                    
                    if mx >= btn_x && mx <= btn_x + 20 &&
                        my >= btn_y && my <= btn_y + 20 {
                        found_close = Some(i);
                        break;
                    }

                    // 2. Проверяем Заголовок (Drag)
                    if mx >= win.x && mx <= win.x + win.width && my >= win.y && my <= win.y + 30 {
                        found_drag = Some(i);
                        break;
                    }
                }

                if let Some(idx) = found_close {
                    // Удаляем окно
                    self.windows.remove(idx);
                    // dragging_idx не ставим
                } else if let Some(idx) = found_drag {
                    // Начинаем тащить
                    self.drag_offset_x = mx - self.windows[idx].x;
                    self.drag_offset_y = my - self.windows[idx].y;
                    
                    // Поднимаем наверх (Z-Order)
                    let win = self.windows.remove(idx);
                    self.windows.push(win);
                    
                    self.dragging_idx = Some(self.windows.len() - 1);
                }
            }
        } else {
            self.dragging_idx = None;
        }
    }

    pub fn get_window_pos(&self, id: usize) -> Option<(usize, usize)> {
        self.windows.iter().find(|w| w.id == id).map(|w| (w.x, w.y))
    }
}