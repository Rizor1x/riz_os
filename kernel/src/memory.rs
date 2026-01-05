use x86_64::{
    structures::paging::{
        PageTable, OffsetPageTable, FrameAllocator, 
        PhysFrame, Size4KiB, Translate
        // Page удалили, так как он не использовался и вызывал warning
    },
    VirtAddr, PhysAddr
};
// PhysFrameRange переехал сюда в x86_64 v0.15
use limine::memory_map::EntryType;


// --- Исправленный Аллокатор ---

pub struct BootInfoFrameAllocator {
    // Limine возвращает слайс ССЫЛОК на Entry (&[&Entry]), а не самих Entry
    memory_map: &'static [&'static limine::memory_map::Entry],
    next: usize,
}

impl BootInfoFrameAllocator {
    // Принимаем правильный тип данных от Limine
    pub unsafe fn init(memory_map: &'static [&'static limine::memory_map::Entry]) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        self.memory_map
            .iter()
            // Разыменовываем двойную ссылку (&&Entry -> &Entry)
            .map(|&entry| entry)
            .filter(|r| r.entry_type == EntryType::USABLE)
            .flat_map(|r| {
                let start = PhysFrame::containing_address(PhysAddr::new(r.base));
                let end = PhysFrame::containing_address(PhysAddr::new(r.base + r.length - 1u64));
                PhysFrame::range_inclusive(start, end)
            })
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub fn translate_addr(addr: u64, physical_memory_offset: u64) -> Option<u64> {
    let virt = VirtAddr::new(addr);
    let offset = VirtAddr::new(physical_memory_offset);
    
    unsafe {
        // 1. Получаем активную таблицу L4
        let level_4_table = active_level_4_table(offset);
        
        // 2. Создаем маппер
        let mapper = OffsetPageTable::new(level_4_table, offset);
        
        // 3. Спрашиваем физический адрес
        mapper.translate_addr(virt).map(|phys| phys.as_u64())
    }
}

// --- Mapper ---

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub unsafe fn init_mapper(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}
