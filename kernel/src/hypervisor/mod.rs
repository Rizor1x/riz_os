pub mod vmcs;
pub mod ept;

use alloc::alloc::Layout;
use alloc::boxed::Box;
use crate::memory;
use crate::HHDM_OFFSET;
use core::sync::atomic::Ordering;
use x86_64::registers::model_specific::Msr;
use x86_64::registers::control::{Cr4, Cr4Flags};

const IA32_FEATURE_CONTROL: u32 = 0x3A;
static mut VMX_LAUNCHED: bool = false;

// Включает VMX и настраивает VMCS. Возвращает Ok, если готово к запуску.
pub unsafe fn init_vm() -> Result<(), &'static str> {
    crate::println!("\n[>] Initializing Hypervisor...");

    // 1. Enable VMX
    let mut cr4 = Cr4::read();
    cr4 |= Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS;
    Cr4::write(cr4);
    let feature_control = Msr::new(IA32_FEATURE_CONTROL).read();
    if (feature_control & 1) != 0 && (feature_control & (1 << 2)) == 0 { return Err("VMX Locked"); }
    if (feature_control & 1) == 0 { Msr::new(IA32_FEATURE_CONTROL).write(feature_control | 5); }
    enable_required_bits();

    // 2. VMXON
    let layout = Layout::from_size_align(4096, 4096).unwrap();
    let vmxon_ptr = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(vmxon_ptr, 0, 4096);
    let rev_id = Msr::new(0x480).read() as u32;
    *(vmxon_ptr as *mut u32) = rev_id;
    let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
    let vmxon_phys = memory::translate_addr(vmxon_ptr as u64, hhdm).unwrap();
    
    let rflags: u64;
    core::arch::asm!("vmxon [{0}]", "pushf", "pop {1}", in(reg) &vmxon_phys, out(reg) rflags, options(nostack, preserves_flags));
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { return Err("VMXON Failed"); }

    // 3. VMCS
    let vmcs_ptr = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(vmcs_ptr, 0, 4096);
    *(vmcs_ptr as *mut u32) = rev_id;
    let vmcs_phys = memory::translate_addr(vmcs_ptr as u64, hhdm).unwrap();
    
    let mut rflags: u64;
    core::arch::asm!("vmclear [{0}]", "pushf", "pop {1}", in(reg) &vmcs_phys, out(reg) rflags, options(nostack, preserves_flags));
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { return Err("VMCLEAR Failed"); }
    core::arch::asm!("vmptrld [{0}]", "pushf", "pop {1}", in(reg) &vmcs_phys, out(reg) rflags, options(nostack, preserves_flags));
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { return Err("VMPTRLD Failed"); }

    // 4. EPT
    let ept_ptr = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(ept_ptr, 0, 4096);
    let ept_phys = memory::translate_addr(ept_ptr as u64, hhdm).unwrap();
    let ept_mgr = Box::leak(Box::new(self::ept::EptManager::new_identity_map(0, 1024*1024*1024)));
    self::vmcs::vmwrite(self::vmcs::fields::EPT_POINTER, ept_mgr.get_pointer()).unwrap();

    // 5. Setup
    if let Err(e) = self::vmcs::setup_host_state() { return Err(e); }
    
    extern "C" { fn vm_exit_entry(); }
    self::vmcs::vmwrite(self::vmcs::fields::HOST_RIP, vm_exit_entry as *const () as u64).unwrap();
    let rsp: u64; core::arch::asm!("mov {}, rsp", out(reg) rsp);
    self::vmcs::vmwrite(self::vmcs::fields::HOST_RSP, rsp).unwrap();

    if let Err(e) = self::vmcs::setup_vm_controls_64bit() { return Err(e); }

    // 6. Guest Code (HLT Loop)
    let guest_mem = alloc::alloc::alloc(layout);
    core::ptr::write_bytes(guest_mem, 0, 4096);
    let code: [u8; 3] = [0xF4, 0xEB, 0xFD]; // HLT; JMP -3
    core::ptr::copy_nonoverlapping(code.as_ptr(), guest_mem, 3);
    
    // Мапим код по адресу 0 для гостя (через EPT 0 -> guest_mem_phys)
    // Но у нас Identity Map на первые 1GB, так что гость видит нашу память 1-в-1.
    // Чтобы это работало корректно, лучше использовать guest_mem_phys как точку входа.
    let guest_phys = memory::translate_addr(guest_mem as u64, hhdm).unwrap();
    if let Err(e) = self::vmcs::setup_guest_state_64bit(guest_phys) { return Err(e); }

    self::vmcs::vmwrite(self::vmcs::fields::GUEST_RFLAGS, 0x202).unwrap();

    VMX_LAUNCHED = false;
    crate::println!("[+] VM Initialized. Ready to run.");
    Ok(())
}

// Выполняет один "кусочек" работы виртуалки (до ближайшего прерывания)
// Возвращает false, если произошла фатальная ошибка
pub unsafe fn tick_vm() -> bool {
    let ret = self::vmcs::run_vm_loop_asm(VMX_LAUNCHED);
    
    if ret != 0 {
        crate::println!("[-] VM Launch/Resume Error!");
        return false;
    }
    
    VMX_LAUNCHED = true;

    // После выхода включаем прерывания, чтобы обновить мышь/клаву
    x86_64::instructions::interrupts::enable();

    let reason = self::vmcs::vmread(0x4402).unwrap_or(0) & 0xFFFF;

    match reason {
        0x01 => true, // Interrupt -> OK, continue next frame
        0x0C => true, // HLT -> OK
        _ => {
            crate::println!("\n[!] VM Exit Reason: {:#x}. Stopping.", reason);
            let err = self::vmcs::vmread(0x4400).unwrap_or(0);
            crate::println!("    Error: {}", err);
            false
        }
    }
}

pub unsafe fn stop_vm() {
    core::arch::asm!("vmxoff", options(nostack, preserves_flags));
    VMX_LAUNCHED = false;
    crate::println!("[<] VMX Stopped.");
}

unsafe fn enable_required_bits() {
    let f0_cr0 = Msr::new(0x486).read(); let f1_cr0 = Msr::new(0x487).read();
    let mut cr0 = x86_64::registers::control::Cr0::read_raw();
    cr0 |= f0_cr0; cr0 &= f1_cr0; x86_64::registers::control::Cr0::write_raw(cr0);

    let f0_cr4 = Msr::new(0x488).read(); let f1_cr4 = Msr::new(0x489).read();
    let mut cr4 = x86_64::registers::control::Cr4::read_raw();
    cr4 |= f0_cr4; cr4 &= f1_cr4; x86_64::registers::control::Cr4::write_raw(cr4);
}