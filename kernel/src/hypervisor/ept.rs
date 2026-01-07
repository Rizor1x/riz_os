use x86_64::structures::paging::{
    PageTable, PageTableFlags, PhysFrame, Size4KiB, Size2MiB
};
use x86_64::PhysAddr;
use alloc::boxed::Box;
use alloc::vec::Vec;
use crate::memory;

// EPT Флаги
const EPT_READ: u64 = 1 << 0;
const EPT_WRITE: u64 = 1 << 1;
const EPT_EXEC: u64 = 1 << 2;
const EPT_MEM_TYPE_WB: u64 = 6 << 3;

#[repr(C, align(4096))]
pub struct EptPageTable {
    entries: [u64; 512],
}

impl EptPageTable {
    pub fn new() -> Box<Self> {
        Box::new(Self { entries: [0; 512] })
    }
}

// Структура, которая держит все таблицы EPT, чтобы они не удалились из памяти
pub struct EptManager {
    pml4: Box<EptPageTable>,
    pdpt: Box<EptPageTable>,
    pd: Box<EptPageTable>,
    pt: Vec<Box<EptPageTable>>, // Таблицы нижнего уровня (4KB)
}

impl EptManager {
    // Создает EPT, которая мапит диапазон Guest Phys [0..size] -> Host Phys [start..start+size]
    pub fn new_identity_map(host_phys_start: u64, size: u64) -> Self {
        let mut pml4 = EptPageTable::new();
        let mut pdpt = EptPageTable::new();
        let mut pd = EptPageTable::new();
        let mut pt_vec = Vec::new();

        // 1. Настраиваем PML4[0] -> PDPT
        let pdpt_phys = memory::translate_addr(pdpt.as_ref() as *const _ as u64, crate::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)).unwrap();
        pml4.entries[0] = pdpt_phys | EPT_READ | EPT_WRITE | EPT_EXEC;

        // 2. Настраиваем PDPT[0] -> PD
        let pd_phys = memory::translate_addr(pd.as_ref() as *const _ as u64, crate::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)).unwrap();
        pdpt.entries[0] = pd_phys | EPT_READ | EPT_WRITE | EPT_EXEC;

        // 3. Заполняем PD (Page Directory) и PT (Page Tables)
        // Каждая запись в PD указывает на таблицу PT (которая покрывает 2MB)
        // Каждая запись в PT указывает на 4KB фрейм
        
        let num_pages = (size + 4095) / 4096;
        let mut pages_mapped = 0;

        for pd_index in 0..512 {
            if pages_mapped >= num_pages { break; }

            // Создаем новую PT
            let mut pt = EptPageTable::new();
            
            for pt_index in 0..512 {
                if pages_mapped >= num_pages { break; }

                let offset = pages_mapped * 4096;
                let host_frame_phys = host_phys_start + offset;
                
                // Мапим: Guest (offset) -> Host (start + offset)
                pt.entries[pt_index] = host_frame_phys | EPT_READ | EPT_WRITE | EPT_EXEC | EPT_MEM_TYPE_WB;
                
                pages_mapped += 1;
            }

            // Добавляем PT в PD
            let pt_phys = memory::translate_addr(pt.as_ref() as *const _ as u64, crate::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)).unwrap();
            pd.entries[pd_index] = pt_phys | EPT_READ | EPT_WRITE | EPT_EXEC;
            
            // Сохраняем PT, чтобы Rust не удалил её
            pt_vec.push(pt);
        }

        Self {
            pml4, pdpt, pd, pt: pt_vec
        }
    }

    // Получить физический адрес корня (PML4) для записи в VMCS
    pub fn get_pointer(&self) -> u64 {
        let virt = self.pml4.as_ref() as *const _ as u64;
        let phys = memory::translate_addr(virt, crate::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)).unwrap();
        phys | 0x1E // Flags: WriteBack, 4-level
    }
}