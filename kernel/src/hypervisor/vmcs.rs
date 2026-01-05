// Обертка над инструкциями VMREAD и VMWRITE

pub unsafe fn vmwrite(field: u64, value: u64) -> Result<(), &'static str> {
    let rflags: u64;
    core::arch::asm!(
        "vmwrite {1}, {0}", // Dest (Field), Source (Value) - Правильно
        "pushf",
        "pop {2}",
        in(reg) value,      // {0}
        in(reg) field,      // {1}
        out(reg) rflags,    // {2}
        options(nostack, preserves_flags)
    );

    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 {
        return Err("VMWRITE Failed");
    }
    Ok(())
}

pub unsafe fn vmread(field: u64) -> Result<u64, &'static str> {
    let mut value: u64;
    let rflags: u64;
    
    core::arch::asm!(
        // --- ИСПРАВЛЕНИЕ ТУТ ---
        // Было: "vmread {0}, {1}"
        // Стало: "vmread {1}, {0}" (Сначала Куда (Value), потом Откуда (Field))
        "vmread {1}, {0}", 
        "pushf",
        "pop {2}",
        in(reg) field,       // {0} - Источник (номер поля)
        lateout(reg) value,  // {1} - Приемник (переменная)
        out(reg) rflags,     // {2}
        options(nostack, preserves_flags)
    );

    if (rflags & 1) != 0 || (rflags & (1 << 6)) != 0 {
        return Err("VMREAD Failed");
    }
    Ok(value)
}

// Константы полей VMCS
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
    
    // Natural-Width Guest State Fields (64-bit on x64)
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
    pub const HOST_IA32_SYSENTER_ESP: u64 = 0x6c10;
    pub const HOST_IA32_SYSENTER_EIP: u64 = 0x6c12;
    pub const HOST_RSP: u64 = 0x6c14;
    pub const HOST_RIP: u64 = 0x6c16;
}