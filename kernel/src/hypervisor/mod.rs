use x86_64::registers::control::{Cr4, Cr4Flags};
use x86_64::registers::model_specific::Msr;

const IA32_FEATURE_CONTROL: u32 = 0x3A;

pub mod vmcs;

pub unsafe fn enable_vmx() -> Result<(), &'static str> {
    // 1. Enable VMXE
    let mut cr4 = Cr4::read();
    cr4 |= Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS;
    Cr4::write(cr4);

    // 2. Feature Control Lock
    let feature_control = Msr::new(IA32_FEATURE_CONTROL).read();
    let lock_bit = (feature_control & 1) != 0;
    let vmx_enabled = (feature_control & (1 << 2)) != 0;

    if lock_bit && !vmx_enabled {
        return Err("VMX locked off by BIOS");
    }
    
    if !lock_bit {
        let new_val = feature_control | (1 << 2) | 1; 
        Msr::new(IA32_FEATURE_CONTROL).write(new_val);
    }

    // 3. CR0/CR4 Fixed Bits
    enable_required_bits();
    
    Ok(())
}

unsafe fn enable_required_bits() {
    let fixed0_cr0 = Msr::new(0x486).read();
    let fixed1_cr0 = Msr::new(0x487).read();
    let mut cr0 = x86_64::registers::control::Cr0::read_raw();
    cr0 |= fixed0_cr0;
    cr0 &= fixed1_cr0;
    x86_64::registers::control::Cr0::write_raw(cr0);

    let fixed0_cr4 = Msr::new(0x488).read();
    let fixed1_cr4 = Msr::new(0x489).read();
    let mut cr4 = x86_64::registers::control::Cr4::read_raw();
    cr4 |= fixed0_cr4;
    cr4 &= fixed1_cr4;
    x86_64::registers::control::Cr4::write_raw(cr4);
}