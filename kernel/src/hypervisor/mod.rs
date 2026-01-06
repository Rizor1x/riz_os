pub mod vmcs;

use alloc::alloc::Layout;
use crate::memory;
use crate::HHDM_OFFSET;
use core::sync::atomic::Ordering;
use x86_64::registers::model_specific::Msr;
use x86_64::registers::control::{Cr4, Cr4Flags};
use x86_64::instructions::port::Port; 

const IA32_FEATURE_CONTROL: u32 = 0x3A;

// --- ФУНКЦИЯ ОПРОСА ВВОДА ---
unsafe fn poll_input() {
    let mut status_port = Port::<u8>::new(0x64);
    let mut data_port = Port::<u8>::new(0x60);

    // Проверяем, есть ли данные в буфере (Бит 0)
    // Мы делаем это в цикле while, чтобы вычитать ВСЁ, что накопилось
    while status_port.read() & 0x01 != 0 {
        let status = status_port.read();
        let data = data_port.read();

        // Бит 5 = 1 (Мышь), Бит 5 = 0 (Клавиатура)
        if status & 0x20 != 0 {
            crate::interrupts::handle_mouse_raw(data);
        } else {
            crate::interrupts::handle_keyboard_raw(data);
        }
    }
}

pub unsafe fn enable_vmx() -> Result<(), &'static str> {
    let mut cr4 = Cr4::read();
    cr4 |= Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS;
    Cr4::write(cr4);

    let feature_control = Msr::new(IA32_FEATURE_CONTROL).read();
    let lock_bit = (feature_control & 1) != 0;
    let vmx_enabled = (feature_control & (1 << 2)) != 0;

    if lock_bit && !vmx_enabled { return Err("VMX locked off by BIOS"); }
    if !lock_bit {
        let new_val = feature_control | (1 << 2) | 1; 
        Msr::new(IA32_FEATURE_CONTROL).write(new_val);
    }
    enable_required_bits();
    Ok(())
}

unsafe fn enable_required_bits() {
    let fixed0_cr0 = Msr::new(0x486).read();
    let fixed1_cr0 = Msr::new(0x487).read();
    let mut cr0 = x86_64::registers::control::Cr0::read_raw();
    cr0 |= fixed0_cr0; cr0 &= fixed1_cr0;
    x86_64::registers::control::Cr0::write_raw(cr0);

    let fixed0_cr4 = Msr::new(0x488).read();
    let fixed1_cr4 = Msr::new(0x489).read();
    let mut cr4 = x86_64::registers::control::Cr4::read_raw();
    cr4 |= fixed0_cr4; cr4 &= fixed1_cr4;
    x86_64::registers::control::Cr4::write_raw(cr4);
}

pub unsafe fn start_vmx() {
    crate::println!("\n[>] Setting up Hypervisor...");

    if let Err(e) = enable_vmx() {
        crate::println!("[-] Setup failed: {}", e); return;
    }
    crate::println!("[+] VMX Enabled.");

    // --- VMXON ---
    let layout = Layout::from_size_align(4096, 4096).unwrap();
    let vmxon_ptr = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(vmxon_ptr, 0, 4096);
    let revision_id = Msr::new(0x480).read() as u32;
    *(vmxon_ptr as *mut u32) = revision_id;

    let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
    let vmxon_phys = memory::translate_addr(vmxon_ptr as u64, hhdm).unwrap();

    let rflags: u64;
    core::arch::asm!("vmxon [{0}]", "pushf", "pop {1}", in(reg) &vmxon_phys, out(reg) rflags, options(nostack, preserves_flags));
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { crate::println!("[-] VMXON Failed"); return; }
    
    crate::println!("[+] Root Operation Active.");

    // --- VMCS ---
    let mut rflags: u64;
    let vmcs_ptr = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(vmcs_ptr, 0, 4096);
    *(vmcs_ptr as *mut u32) = revision_id;
    let vmcs_phys = memory::translate_addr(vmcs_ptr as u64, hhdm).unwrap();

    core::arch::asm!("vmclear [{0}]", "pushf", "pop {1}", in(reg) &vmcs_phys, out(reg) rflags, options(nostack, preserves_flags));
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { crate::println!("[-] VMCLEAR Failed"); return; }
    
    core::arch::asm!("vmptrld [{0}]", "pushf", "pop {1}", in(reg) &vmcs_phys, out(reg) rflags, options(nostack, preserves_flags));
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { crate::println!("[-] VMCS Load Failed"); return; }

    // --- EPT ---
    let ept_ptr = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(ept_ptr, 0, 4096);
    let ept_phys = memory::translate_addr(ept_ptr as u64, hhdm).unwrap();
    self::vmcs::vmwrite(self::vmcs::fields::EPT_POINTER, ept_phys | 0x1E).unwrap();

    // --- SETUP ---
    use crate::hypervisor::vmcs;
    if let Err(e) = vmcs::setup_host_state() { crate::println!("[-] Host Fail: {}", e); return; }
    
    extern "C" { fn vm_exit_entry(); }
    vmcs::vmwrite(vmcs::fields::HOST_RIP, vm_exit_entry as *const () as u64).unwrap();

    if let Err(e) = vmcs::setup_vm_controls_64bit() { crate::println!("[-] Controls Fail: {}", e); return; }

    // --- GUEST CODE (HLT LOOP) ---
    // F4 (HLT), EB FD (JMP -3)
    let guest_mem = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(guest_mem, 0, 4096);
    let code: [u8; 3] = [0xF4, 0xEB, 0xFD]; 
    core::ptr::copy_nonoverlapping(code.as_ptr(), guest_mem, 3);
    
    let guest_entry = guest_mem as u64;
    if let Err(e) = vmcs::setup_guest_state_64bit(guest_entry) { crate::println!("[-] Guest Fail: {}", e); return; }
    
    // Включаем прерывания внутри гостя
    vmcs::vmwrite(vmcs::fields::GUEST_RFLAGS, 0x202).unwrap();

    // --- LOOP ---
    crate::println!("\n[!!!] HYPERVISOR RUNNING. Press ESC to stop.");
    
    let mut launched = false;
    use crate::interrupts::STOP_VM;
    STOP_VM.store(false, Ordering::Relaxed);

    loop {
        // 1. ОПРАШИВАЕМ ВВОД ВРУЧНУЮ
        // Это самое важное изменение. Мы насильно читаем порты.
        poll_input();

        // 2. Проверка выхода
        if STOP_VM.load(Ordering::Relaxed) {
            crate::println!("\n[<] Stopping VM.");
            break;
        }

        // 3. Запуск VM
        vmcs::run_vm_loop(launched);
        launched = true;

        // 4. После выхода: Включаем прерывания (чтобы сбросить контроллер)
        x86_64::instructions::interrupts::enable(); 
        
        // Маленькая пауза не нужна, если мы делаем poll_input, но оставим для стабильности
        // x86_64::instructions::nop();

        let reason = vmcs::vmread(0x4402).unwrap_or(0) & 0xFFFF;

        match reason {
            0x01 => {
                // External Interrupt.
                // Мы вызовем poll_input() в начале следующего цикла, 
                // так что данные точно будут прочитаны.
                continue;
            },
            0x0C => {
                // HLT. Гость спит.
                // Тоже можно опросить ввод, пока он спит.
                poll_input();
                continue;
            },
            _ => {
                crate::println!("\n[!] Exit Reason: {:#x}", reason);
                break;
            }
        }
    }

    core::arch::asm!("vmxoff", options(nostack, preserves_flags));
}