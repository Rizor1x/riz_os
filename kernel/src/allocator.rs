use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};
use linked_list_allocator::LockedHeap;

// Глобальный аллокатор.
// LockedHeap сам использует спинлоки, так что он thread-safe.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Настройки кучи
pub const HEAP_START: usize = 0x_4444_4444_0000; // Просто произвольный адрес, который точно свободен
pub const HEAP_SIZE: usize = 128 * 1024 * 1024; // 

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    
    // 1. Создаем диапазон страниц для кучи
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // 2. Маппим эти страницы на физические фреймы
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        
        // Флаги: Чтение | Запись
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        };
    }

    // 3. Инициализируем сам аллокатор
    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    Ok(())
}

// Обязательный обработчик ошибок аллокации (если память кончится)
#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}