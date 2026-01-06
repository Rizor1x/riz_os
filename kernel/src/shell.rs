use alloc::string::String;
use crate::fs::TarFileSystem;
use spin::Mutex;
use lazy_static::lazy_static;
use raw_cpuid::CpuId;

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
    if command.starts_with("cat ") {
        let filename = &command[4..];
        if let Some(fs) = &*FILESYSTEM.lock() {
            if let Some(data) = fs.read_file(filename) {
                if let Ok(text) = core::str::from_utf8(data) {
                    println!("\nContent of {}:\n----------------\n{}\n----------------", filename, text);
                } else {
                    println!("\nFile contains binary data.");
                }
            } else {
                println!("\nFile not found: {}", filename);
            }
        } else {
            println!("\nFilesystem not initialized!");
        }
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
        },
        "ver" => println!("\nRizOS v0.1.0"),
        "ls" => {
            println!();
            if let Some(fs) = &*FILESYSTEM.lock() {
                println!("Files on disk.tar:");
                for (name, size) in fs.list_files() {
                    println!(" - {} ({} bytes)", name, size);
                }
            } else {
                println!("Filesystem not initialized!");
            }
        },
        "clear" => println!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n"),
        "cpu" => {
            let cpuid = CpuId::new();
            if let Some(v) = cpuid.get_vendor_info() { println!("Vendor: {}", v.as_str()); }
            if let Some(brand) = cpuid.get_processor_brand_string() {
                println!("Model: {}", brand.as_str().trim());
            }
            if let Some(f) = cpuid.get_feature_info() {
                if f.has_vmx() { println!("[+] VMX Supported!"); } else { println!("[-] VMX Not Supported"); }
            }
        },
        
        "vmxon" => {
            // Вызываем нашу функцию из гипервизора
            // Обрати внимание: функция unsafe, так как работает с железом
            unsafe {
                crate::hypervisor::start_vmx();
            }
        },
        "" => {},
        _ => println!("\nUnknown command: '{}'", command),
    }
}