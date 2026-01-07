use alloc::vec::Vec;

// Магическая сигнатура "HdrS" (Header Start)
const LINUX_BOOT_MAGIC: u32 = 0x53726448; 

#[repr(C, packed)]
pub struct LinuxSetupHeader {
    pub setup_sects: u8,      // 0x1F1
    pub root_flags: u16,      // 0x1F2
    pub syssize: u32,         // 0x1F4
    pub ram_size: u16,        // 0x1F8
    pub vid_mode: u16,        // 0x1FA
    pub root_dev: u16,        // 0x1FC
    pub boot_flag: u16,       // 0x1FE (0xAA55)
    pub jump: u16,            // 0x200
    pub header: u32,          // 0x202 ("HdrS")
    pub version: u16,         // 0x206
    pub realmode_swtch: u32,  // 0x208
    pub start_sys_seg: u16,   // 0x20C
    pub kernel_version: u16,  // 0x20E
    pub type_of_loader: u8,   // 0x210
    pub loadflags: u8,        // 0x211
    pub setup_move_size: u16, // 0x212
    pub code32_start: u32,    // 0x214
    pub ramdisk_image: u32,   // 0x218
    pub ramdisk_size: u32,    // 0x21C
    // ... остальные поля пока не важны
}

pub struct LinuxKernel {
    data: Vec<u8>,
}

impl LinuxKernel {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Проверяет, является ли файл валидным ядром Linux
    pub fn parse_header(&self) -> Result<(), &'static str> {
        if self.data.len() < 1024 {
            return Err("File too small");
        }

        // Заголовок начинается со смещения 0x1F1.
        // Но сигнатура "HdrS" лежит по адресу 0x202.
        
        // Читаем магическое число вручную по смещению 514 (0x202)
        let magic_ptr = unsafe { self.data.as_ptr().add(0x202) as *const u32 };
        let magic = unsafe { core::ptr::read_unaligned(magic_ptr) };

        if magic != LINUX_BOOT_MAGIC {
            return Err("Invalid Magic: Not a Linux Kernel!");
        }

        // Читаем версию протокола (смещение 0x206)
        let version_ptr = unsafe { self.data.as_ptr().add(0x206) as *const u16 };
        let version = unsafe { core::ptr::read_unaligned(version_ptr) };

        crate::println!("[+] Valid Linux Kernel detected!");
        crate::println!("    Protocol Version: {}.{}", version >> 8, version & 0xFF);
        
        // Читаем размер Setup Code (в секторах по 512 байт)
        let setup_sects_ptr = unsafe { self.data.as_ptr().add(0x1F1) as *const u8 };
        let setup_sects = unsafe { *setup_sects_ptr } as usize;
        let setup_sects = if setup_sects == 0 { 4 } else { setup_sects };
        
        crate::println!("    Setup Sectors: {}", setup_sects);
        
        Ok(())
    }
}