use x86_64::registers::control::{Cr0, Cr3, Cr4};
use x86_64::registers::segmentation::{CS, DS, ES, FS, GS, SS, Segment};
use x86_64::registers::model_specific::Msr;
use x86_64::instructions::tables::{sgdt, sidt};

pub unsafe fn vmwrite(field: u64, value: u64) -> Result<(), &'static str> {
    let rflags: u64;
    core::arch::asm!(
        "vmwrite {1}, {0}", 
        "pushf",
        "pop {2}",
        in(reg) value, in(reg) field, out(reg) rflags,
        options(nostack, preserves_flags)
    );
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { Err("VMWRITE Failed") } else { Ok(()) }
}

pub unsafe fn vmread(field: u64) -> Result<u64, &'static str> {
    let mut value: u64;
    let rflags: u64;
    core::arch::asm!(
        "vmread {1}, {0}", 
        "pushf",
        "pop {2}",
        in(reg) field, lateout(reg) value, out(reg) rflags,
        options(nostack, preserves_flags)
    );
    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 { Err("VMREAD Failed") } else { Ok(value) }
}

unsafe fn adjust_vmx_controls(control: u32, msr: u32) -> u32 {
    let msr_val = Msr::new(msr).read();
    let allowed_0 = msr_val as u32;
    let allowed_1 = (msr_val >> 32) as u32;
    let mut effective = control;
    effective |= allowed_0;
    effective &= allowed_1;
    effective
}

const IA32_VMX_PINBASED_CTLS: u32 = 0x481;
const IA32_VMX_PROCBASED_CTLS: u32 = 0x482;
const IA32_VMX_EXIT_CTLS: u32 = 0x483;
const IA32_VMX_ENTRY_CTLS: u32 = 0x484;

// --- НАСТРОЙКА ХОСТА (RizOS) ---
pub unsafe fn setup_host_state() -> Result<(), &'static str> {
    use fields::*;

    // 1. CR0, CR3, CR4
    vmwrite(HOST_CR0, Cr0::read_raw())?;
    vmwrite(HOST_CR3, Cr3::read().0.start_address().as_u64())?;
    vmwrite(HOST_CR4, Cr4::read_raw())?;

    // 2. Сегменты
    vmwrite(HOST_CS_SELECTOR, CS::get_reg().0 as u64)?;
    vmwrite(HOST_SS_SELECTOR, SS::get_reg().0 as u64)?;
    vmwrite(HOST_DS_SELECTOR, DS::get_reg().0 as u64)?;
    vmwrite(HOST_ES_SELECTOR, ES::get_reg().0 as u64)?;
    vmwrite(HOST_FS_SELECTOR, FS::get_reg().0 as u64)?;
    vmwrite(HOST_GS_SELECTOR, GS::get_reg().0 as u64)?;
    
    let tr: u16;
    core::arch::asm!("str {0:x}", out(reg) tr);
    vmwrite(HOST_TR_SELECTOR, tr as u64)?;

    // 3. Базы (Bases)
    vmwrite(HOST_FS_BASE, Msr::new(0xC0000100).read())?;
    vmwrite(HOST_GS_BASE, Msr::new(0xC0000101).read())?;
    
    let gdtr = sgdt();
    let idtr = sidt();
    vmwrite(HOST_GDTR_BASE, gdtr.base.as_u64())?;
    vmwrite(HOST_IDTR_BASE, idtr.base.as_u64())?;
    
    // ИСПОЛЬЗУЕМ ФУНКЦИЮ ИЗ GDT.RS
    let tss_base = crate::gdt::get_tss_address();
    vmwrite(HOST_TR_BASE, tss_base)?; 

    // 4. Sysenter
    vmwrite(HOST_IA32_SYSENTER_CS, Msr::new(0x174).read())?;
    vmwrite(HOST_IA32_SYSENTER_ESP, Msr::new(0x175).read())?;
    vmwrite(HOST_IA32_SYSENTER_EIP, Msr::new(0x176).read())?;

    Ok(())
}

// --- НАСТРОЙКА КОНТРОЛОВ (64-BIT GUEST) ---
pub unsafe fn setup_vm_controls_64bit() -> Result<(), &'static str> {
    use fields::*;

    let pin = adjust_vmx_controls(1, IA32_VMX_PINBASED_CTLS);
    vmwrite(PIN_BASED_VM_EXEC_CONTROL, pin as u64)?;

    // Secondary Controls (EPT не включаем пока, чтобы не усложнять)
    // Оставляем Unrestricted=0, EPT=0. Будем запускать чистого 64-bit гостя.
    let proc = adjust_vmx_controls(0, IA32_VMX_PROCBASED_CTLS);
    vmwrite(CPU_BASED_VM_EXEC_CONTROL, proc as u64)?;

    // Exit: Host Address Space Size (Bit 9) = 1 (Host is 64-bit)
    let exit = adjust_vmx_controls(1 << 9, IA32_VMX_EXIT_CTLS);
    vmwrite(VM_EXIT_CONTROLS, exit as u64)?;

    // Entry: IA-32e Mode Guest (Bit 9) = 1 (GUEST IS 64-BIT!)
    let entry = adjust_vmx_controls(1 << 9, IA32_VMX_ENTRY_CTLS);
    vmwrite(VM_ENTRY_CONTROLS, entry as u64)?;

    Ok(())
}

// --- НАСТРОЙКА ГОСТЯ (КОПИЯ ХОСТА) ---
pub unsafe fn setup_guest_state_64bit(entry_point: u64) -> Result<(), &'static str> {
    use fields::*;

    // 1. CR0, CR4, CR3 (Копируем от хоста)
    vmwrite(GUEST_CR0, Cr0::read_raw())?;
    vmwrite(GUEST_CR4, Cr4::read_raw())?;
    vmwrite(GUEST_CR3, Cr3::read().0.start_address().as_u64())?;

    // 2. СЕГМЕНТЫ (ИСПРАВЛЕННАЯ ЛОГИКА)
    
    // CS (Code Segment) - Должен быть валидным 64-bit Code
    // Selector 8 (Kernel Code), Base 0, Limit FFFFFFFF, AR 0xA09B (Long Mode Code)
    vmwrite(GUEST_CS_SELECTOR, 8)?;
    vmwrite(GUEST_CS_BASE, 0)?;
    vmwrite(GUEST_CS_LIMIT, 0xFFFFFFFF)?;
    vmwrite(GUEST_CS_AR_BYTES, 0xA09B)?; 

    // SS (Stack Segment) - Должен быть валидным Writable Data
    // Selector 16 (Kernel Data), Base 0, Limit FFFFFFFF, AR 0xC093 (Writable)
    vmwrite(GUEST_SS_SELECTOR, 16)?;
    vmwrite(GUEST_SS_BASE, 0)?;
    vmwrite(GUEST_SS_LIMIT, 0xFFFFFFFF)?;
    vmwrite(GUEST_SS_AR_BYTES, 0xC093)?;

    // DS, ES, FS, GS - ДЕЛАЕМ ИХ "UNUSABLE" (Неиспользуемыми)
    // Это лайфхак, чтобы процессор не проверял их валидность.
    // В 64-битном режиме это допустимо.
    for &seg in &[GUEST_DS_SELECTOR, GUEST_ES_SELECTOR, GUEST_FS_SELECTOR, GUEST_GS_SELECTOR] {
        vmwrite(seg, 0)?; // Selector 0 (Null)
    }
    for &base in &[GUEST_DS_BASE, GUEST_ES_BASE, GUEST_FS_BASE, GUEST_GS_BASE] {
        vmwrite(base, 0)?;
    }
    for &limit in &[GUEST_DS_LIMIT, GUEST_ES_LIMIT, GUEST_FS_LIMIT, GUEST_GS_LIMIT] {
        vmwrite(limit, 0)?;
    }
    // AR Bytes: Bit 16 (0x10000) = Unusable
    for &ar in &[GUEST_DS_AR_BYTES, GUEST_ES_AR_BYTES, GUEST_FS_AR_BYTES, GUEST_GS_AR_BYTES] {
        vmwrite(ar, 0x10000)?; 
    }
    
    // TR (Task Register) - Обязателен и должен быть валидным
    let tr_base = crate::gdt::get_tss_address();
    vmwrite(GUEST_TR_SELECTOR, 0x20)?; // 0x20 - это индекс TSS в нашей GDT (4-й сегмент: null, code, data, tss)
    vmwrite(GUEST_TR_BASE, tr_base)?;
    vmwrite(GUEST_TR_LIMIT, 0xFFFF)?;
    vmwrite(GUEST_TR_AR_BYTES, 0x008B)?; // 32-bit TSS (Busy)

    // LDTR - Выключаем
    vmwrite(GUEST_LDTR_SELECTOR, 0)?;
    vmwrite(GUEST_LDTR_BASE, 0)?;
    vmwrite(GUEST_LDTR_LIMIT, 0)?;
    vmwrite(GUEST_LDTR_AR_BYTES, 0x10000)?; // Unusable

    // 3. GDTR / IDTR
    let gdtr = sgdt();
    let idtr = sidt();
    vmwrite(GUEST_GDTR_BASE, gdtr.base.as_u64())?;
    vmwrite(GUEST_GDTR_LIMIT, gdtr.limit as u64)?;
    vmwrite(GUEST_IDTR_BASE, idtr.base.as_u64())?;
    vmwrite(GUEST_IDTR_LIMIT, idtr.limit as u64)?;

    // 4. MSRs (EFER)
    // Важно: LMA (Long Mode Active) и LME (Long Mode Enable) должны быть 1
    vmwrite(GUEST_IA32_EFER, Msr::new(0xC0000080).read())?;

    // 5. RIP, RSP, RFLAGS
    vmwrite(GUEST_RIP, entry_point)?;
    vmwrite(GUEST_RSP, 0)?; // Стек 0 - это ок для теста, но потом надо выделить
    vmwrite(GUEST_RFLAGS, 0x2)?; 

    // 6. Misc
    vmwrite(GUEST_ACTIVITY_STATE, 0)?;
    vmwrite(GUEST_INTERRUPTIBILITY_INFO, 0)?;
    vmwrite(GUEST_SYSENTER_CS, 0)?;
    vmwrite(GUEST_SYSENTER_ESP, 0)?;
    vmwrite(GUEST_SYSENTER_EIP, 0)?;
    vmwrite(GUEST_VMCS_LINK_PTR, 0xFFFFFFFFFFFFFFFF)?;

    Ok(())
}

// --- CONSTANTS ---
pub mod fields {
    // 16-Bit Guest State Fields
    pub const GUEST_ES_SELECTOR: u64 = 0x0800;
    pub const GUEST_CS_SELECTOR: u64 = 0x0802;
    pub const GUEST_SS_SELECTOR: u64 = 0x0804;
    pub const GUEST_DS_SELECTOR: u64 = 0x0806;
    pub const GUEST_FS_SELECTOR: u64 = 0x0808;
    pub const GUEST_GS_SELECTOR: u64 = 0x080a;
    pub const GUEST_LDTR_SELECTOR: u64 = 0x080c;
    pub const GUEST_TR_SELECTOR: u64 = 0x080e;

    // 16-Bit Host State Fields
    pub const HOST_ES_SELECTOR: u64 = 0x0c00;
    pub const HOST_CS_SELECTOR: u64 = 0x0c02;
    pub const HOST_SS_SELECTOR: u64 = 0x0c04;
    pub const HOST_DS_SELECTOR: u64 = 0x0c06;
    pub const HOST_FS_SELECTOR: u64 = 0x0c08;
    pub const HOST_GS_SELECTOR: u64 = 0x0c0a;
    pub const HOST_TR_SELECTOR: u64 = 0x0c0c;

    // 64-Bit Guest State Fields
    pub const GUEST_VMCS_LINK_PTR: u64 = 0x2800;
    pub const GUEST_IA32_DEBUGCTL: u64 = 0x2802;
    pub const GUEST_IA32_EFER: u64 = 0x2806;

    // 32-Bit Control Fields
    pub const PIN_BASED_VM_EXEC_CONTROL: u64 = 0x4000;
    pub const CPU_BASED_VM_EXEC_CONTROL: u64 = 0x4002;
    pub const EXCEPTION_BITMAP: u64 = 0x4004;
    pub const PAGE_FAULT_ERROR_CODE_MASK: u64 = 0x4006;
    pub const PAGE_FAULT_ERROR_CODE_MATCH: u64 = 0x4008;
    pub const CR3_TARGET_COUNT: u64 = 0x400a;
    pub const VM_EXIT_CONTROLS: u64 = 0x400c;
    pub const VM_EXIT_MSR_STORE_COUNT: u64 = 0x400e;
    pub const VM_EXIT_MSR_LOAD_COUNT: u64 = 0x4010;
    pub const VM_ENTRY_CONTROLS: u64 = 0x4012;
    pub const VM_ENTRY_MSR_LOAD_COUNT: u64 = 0x4014;
    pub const VM_ENTRY_INTR_INFO_FIELD: u64 = 0x4016;
    
    // Natural-Width Guest State Fields
    pub const GUEST_CR0: u64 = 0x6800;
    pub const GUEST_CR3: u64 = 0x6802;
    pub const GUEST_CR4: u64 = 0x6804;
    pub const GUEST_ES_BASE: u64 = 0x6806;
    pub const GUEST_CS_BASE: u64 = 0x6808;
    pub const GUEST_SS_BASE: u64 = 0x680a;
    pub const GUEST_DS_BASE: u64 = 0x680c;
    pub const GUEST_FS_BASE: u64 = 0x680e;
    pub const GUEST_GS_BASE: u64 = 0x6810;
    pub const GUEST_LDTR_BASE: u64 = 0x6812;
    pub const GUEST_TR_BASE: u64 = 0x6814;
    pub const GUEST_GDTR_BASE: u64 = 0x6816;
    pub const GUEST_IDTR_BASE: u64 = 0x6818;
    pub const GUEST_DR7: u64 = 0x681a;
    pub const GUEST_RSP: u64 = 0x681c;
    pub const GUEST_RIP: u64 = 0x681e;
    pub const GUEST_RFLAGS: u64 = 0x6820;

    // Natural-Width Host State Fields
    pub const HOST_CR0: u64 = 0x6c00;
    pub const HOST_CR3: u64 = 0x6c02;
    pub const HOST_CR4: u64 = 0x6c04;
    pub const HOST_FS_BASE: u64 = 0x6c06;
    pub const HOST_GS_BASE: u64 = 0x6c08;
    pub const HOST_TR_BASE: u64 = 0x6c0a;
    pub const HOST_GDTR_BASE: u64 = 0x6c0c;
    pub const HOST_IDTR_BASE: u64 = 0x6c0e;
    pub const HOST_IA32_SYSENTER_CS: u64 = 0x4c00; 
    pub const HOST_IA32_SYSENTER_ESP: u64 = 0x6c10;
    pub const HOST_IA32_SYSENTER_EIP: u64 = 0x6c12;
    pub const HOST_RSP: u64 = 0x6c14;
    pub const HOST_RIP: u64 = 0x6c16;

    // Limits & AR
    pub const GUEST_ES_AR_BYTES: u64 = 0x4814;
    pub const GUEST_CS_AR_BYTES: u64 = 0x4816;
    pub const GUEST_SS_AR_BYTES: u64 = 0x4818;
    pub const GUEST_DS_AR_BYTES: u64 = 0x481a;
    pub const GUEST_FS_AR_BYTES: u64 = 0x481c;
    pub const GUEST_GS_AR_BYTES: u64 = 0x481e;
    pub const GUEST_LDTR_AR_BYTES: u64 = 0x4820;
    pub const GUEST_TR_AR_BYTES: u64 = 0x4822;
    
    pub const GUEST_ES_LIMIT: u64 = 0x4800;
    pub const GUEST_CS_LIMIT: u64 = 0x4802;
    pub const GUEST_SS_LIMIT: u64 = 0x4804;
    pub const GUEST_DS_LIMIT: u64 = 0x4806;
    pub const GUEST_FS_LIMIT: u64 = 0x4808;
    pub const GUEST_GS_LIMIT: u64 = 0x480a;
    pub const GUEST_LDTR_LIMIT: u64 = 0x480c;
    pub const GUEST_TR_LIMIT: u64 = 0x480e;
    pub const GUEST_GDTR_LIMIT: u64 = 0x4810;
    pub const GUEST_IDTR_LIMIT: u64 = 0x4812;

    pub const GUEST_INTERRUPTIBILITY_INFO: u64 = 0x4824;
    pub const GUEST_ACTIVITY_STATE: u64      = 0x4826;
    pub const GUEST_SYSENTER_CS: u64         = 0x482A;
    pub const GUEST_SYSENTER_ESP: u64 = 0x6822;
    pub const GUEST_SYSENTER_EIP: u64 = 0x6824;
    
    pub const EPT_POINTER: u64 = 0x201A;
}