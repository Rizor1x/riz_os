use x86_64::structures::paging::{
    FrameAllocator, PhysFrame, Size4KiB,
};
use x86_64::PhysAddr;
use limine::memory_map::EntryType;

/// Аллокатор, который создает PhysFrame из карты памяти Limine.
pub struct BootInfoFrameAllocator {
    // Мы храним копию карты памяти (или итератор), чтобы знать, что свободно
    memory_map: &'static [limine::memory_map::Entry],
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Создает аллокатор на основе карты памяти
    /// unsafe, так как вызывающий должен гарантировать, что карта памяти валидна
    pub unsafe fn init(memory_map: &'static [limine::memory_map::Entry]) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Возвращает итератор по всем свободным фреймам (страницам)
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        // 1. Берем карту памяти
        self.memory_map
            .iter()
            // 2. Оставляем только регионы типа USABLE
            .filter(|r| r.entry_type == EntryType::USABLE)
            // 3. Превращаем регион (start, len) в набор адресов страниц
            .flat_map(|r| {
                // Выравниваем адреса по 4096 байт
                let start_addr = r.base;
                let end_addr = r.base + r.length;
                
                let start_frame = PhysFrame::containing_address(PhysAddr::new(start_addr));
                let end_frame = PhysFrame::containing_address(PhysAddr::new(end_addr - 1));
                
                // Создаем диапазон фреймов
                PhysFrame::range_inclusive(start_frame, end_frame)
            })
    }
}

// Реализуем трейт FrameAllocator из библиотеки x86_64.
// Это позволит нам использовать этот аллокатор в стандартных функциях ядра.
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // Мы используем простую стратегию: каждый раз заново проходим и ищем N-ный свободный фрейм.
        // Это медленно (O(N)), но для инициализации ядра этого достаточно.
        // Позже мы заменим это на быстрый аллокатор.
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}