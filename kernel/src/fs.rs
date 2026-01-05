use alloc::vec::Vec;
use alloc::string::String;
use core::str;

// Заголовок TAR (512 байт)
#[repr(C, align(512))]
struct TarHeader {
    name: [u8; 100],     // Имя файла
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],      // Размер файла (в восьмеричной системе ASCII!)
    mtime: [u8; 12],
    checksum: [u8; 8],
    typeflag: u8,        // Тип (0 = файл, 5 = папка)
    linkname: [u8; 100],
    magic: [u8; 6],      // "ustar\0"
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    prefix: [u8; 155],
    pad: [u8; 12],
}

pub struct TarFileSystem {
    base_addr: *const u8,
    size: u64,
}

unsafe impl Send for TarFileSystem {}
unsafe impl Sync for TarFileSystem {}

impl TarFileSystem {
    /// Создает новую FS из адреса памяти и размера
    pub unsafe fn new(addr: *const u8, size: u64) -> Self {
        Self { base_addr: addr, size }
    }

    /// Возвращает список файлов (имя, размер)
    pub fn list_files(&self) -> Vec<(String, usize)> {
        let mut files = Vec::new();
        let mut offset = 0;

        while offset < self.size {
            let header_ptr = unsafe { self.base_addr.add(offset as usize) as *const TarHeader };
            let header = unsafe { &*header_ptr };

            // Проверка на конец архива (пустой заголовок)
            if header.name[0] == 0 {
                break;
            }

            // Парсим размер файла (он хранится как текст "000123")
            let size = parse_octal(&header.size);
            
            // Парсим имя
            let name = parse_name(&header.name);

            // Если это обычный файл (typeflag '0' или '\0'), добавляем
            if header.typeflag == b'0' || header.typeflag == 0 {
                files.push((name, size));
            }

            // Смещаемся к следующему заголовку
            // 512 байт заголовок + размер файла (выровненный по 512)
            let aligned_size = (size + 511) & !511;
            offset += 512 + aligned_size as u64;
        }
        files
    }

    /// Читает содержимое файла по имени
    pub fn read_file(&self, filename: &str) -> Option<&[u8]> {
        let mut offset = 0;

        while offset < self.size {
            let header_ptr = unsafe { self.base_addr.add(offset as usize) as *const TarHeader };
            let header = unsafe { &*header_ptr };

            if header.name[0] == 0 { break; }

            let size = parse_octal(&header.size);
            let name = parse_name(&header.name);

            // Если нашли файл
            if name.trim() == filename.trim() {
                // Данные лежат сразу после заголовка (512 байт)
                let data_ptr = unsafe { self.base_addr.add(offset as usize + 512) };
                return Some(unsafe { core::slice::from_raw_parts(data_ptr, size) });
            }

            let aligned_size = (size + 511) & !511;
            offset += 512 + aligned_size as u64;
        }
        None
    }
}

// Вспомогательная функция: парсинг восьмеричного числа из ASCII
fn parse_octal(data: &[u8]) -> usize {
    let mut result = 0;
    for &byte in data {
        if byte < b'0' || byte > b'7' { break; }
        result = result * 8 + (byte - b'0') as usize;
    }
    result
}

// Вспомогательная функция: парсинг имени (C-string до нулевого байта)
fn parse_name(data: &[u8]) -> String {
    let len = data.iter().position(|&x| x == 0).unwrap_or(data.len());
    str::from_utf8(&data[0..len]).unwrap_or("<invalid>").into()
}