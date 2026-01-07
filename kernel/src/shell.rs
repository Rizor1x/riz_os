use alloc::string::String;
use crate::fs::TarFileSystem;
use spin::Mutex;
use lazy_static::lazy_static;
use raw_cpuid::CpuId;
use crate::graphics::SCREEN;
use crate::linux::LinuxKernel;

static mut VM_RUNNING: bool = false;

// Глобальный буфер для текста команды
lazy_static! {
    static ref COMMAND_BUFFER: Mutex<String> = Mutex::new(String::new());
}

// Ссылка на файловую систему (чтобы шелл мог читать файлы)
pub static FILESYSTEM: Mutex<Option<TarFileSystem>> = Mutex::new(None);

pub fn handle_keystroke(c: char) {
    let mut buffer = COMMAND_BUFFER.lock();

    match c {
        '\n' => {
            // Enter: Выполняем команду
            println!(); // Новая строка визуально
            execute_command(&buffer);
            buffer.clear();
            print!("> ");
        },
        '\x08' => {
            // Backspace
            if !buffer.is_empty() {
                buffer.pop();
                print!("{}", c); // writer.rs сам сотрет символ
            }
        },
        _ => {
            // Обычная буква
            buffer.push(c);
            print!("{}", c);
        }
    }
}

fn execute_command(command: &str) {
    let command = command.trim();
    
    // Команда чтения файла
    if command.starts_with("cat ") {
        let filename = &command[4..];
        if let Some(fs) = &*crate::shell::FILESYSTEM.lock() {
            if let Some(data) = fs.read_file(filename) {
                if let Ok(text) = core::str::from_utf8(data) {
                    println!("\n--- {} ---\n{}\n-----------", filename, text);
                } else { println!("\n[Binary File, Size: {} bytes]", data.len()); }
            } else { println!("\nFile not found."); }
        } else { println!("\nNo FS."); }
        return;
    }

    match command {
        "help" => {
            println!("\nRizOS Help:");
            println!("  help  - Show this message");
            println!("  ver   - Show OS version");
            println!("  ls    - List files");
            println!("  cat   - Read file");
            println!("  clear - Clear screen");
            println!("  cpu   - CPU Info");
            println!("  vmxon   - VM Hypervisor Info");
            println!("  bootlinux - Load vmlinuz into RAM");
        },
        "ver" => println!("\nRizOS v0.3 (Hypervisor Ready)"),
        "clear" => { println!("\n\n\n\n\n\n\n\n\n\n\n\n"); },
        "ls" => {
            println!();
            if let Some(fs) = &*crate::shell::FILESYSTEM.lock() {
                for (name, size) in fs.list_files() { println!("- {} ({} b)", name, size); }
            }
        },
        "cpu" => {
            use raw_cpuid::CpuId;
            let cpuid = CpuId::new();
            if let Some(v) = cpuid.get_vendor_info() { println!("Vendor: {}", v.as_str()); }
            if let Some(f) = cpuid.get_feature_info() {
                if f.has_vmx() { println!("[+] VMX Supported"); } else { println!("[-] No VMX"); }
            }
        },
        // ЗАПУСК ГИПЕРВИЗОРА (Вручную, на Ядре 0)
        "vmxon" => {
            use core::sync::atomic::Ordering;
            // Просто поднимаем флаг. Main.rs увидит это и запустит виртуалку.
            if !crate::interrupts::VM_ACTIVE.load(Ordering::Relaxed) {
                crate::interrupts::VM_ACTIVE.store(true, Ordering::Relaxed);
                println!("Signal sent to Kernel: Start VM.");
            } else {
                println!("VM is already active.");
            }
        },
        // ЗАГРУЗКА ЯДРА LINUX
        "bootlinux" => {
            println!("\nSearching for 'vmlinuz'...");
            if let Some(fs) = &*crate::shell::FILESYSTEM.lock() {
                if let Some(data) = fs.read_file("vmlinuz") {
                    println!("[+] Kernel found! Size: {} bytes", data.len());
                    
                    // Сохраняем в глобальный буфер
                    let mut kbuf = crate::LINUX_KERNEL_BUFFER.lock();
                    *kbuf = alloc::vec::Vec::from(data);
                    
                    println!("[+] Kernel loaded into Host RAM.");
                    println!("    Run 'vmxon' to launch it in VM.");
                } else {
                    println!("[-] File 'vmlinuz' not found.");
                }
            } else {
                println!("[-] FS not mounted.");
            }
        },
        "" => {},
        _ => println!("\nUnknown: '{}'", command),
    }
}