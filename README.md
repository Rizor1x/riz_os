# RizOS 🦀

Experimental x86_64 Operating System written in Rust.
Focuses on Async/Await multitasking, UEFI booting, and Hardware Virtualization (Hypervisor).

## 🚀 Current Status: v0.1 (Stable Base)

### ✅ Implemented Features (Core)
- [x] **Bootloader:** Limine (UEFI) with Limine Boot Protocol.
- [x] **Kernel:** 64-bit Higher Half Kernel written in Rust (no_std).
- [x] **Memory Management:**
    - Physical Memory Manager (Frame Allocator).
    - Virtual Memory Manager (Page Tables & HHDM).
    - Heap Allocator (Linked List, `Vec`/`Box` support).
- [x] **Interrupts & Safety:**
    - IDT (Interrupt Descriptor Table).
    - GDT (Global Descriptor Table).
    - TSS (Task State Segment) with Double Fault stack.
    - PIC 8259 Remapping.
- [x] **Multitasking:**
    - Cooperative Async/Await Executor.
    - Non-blocking drivers.

### 🖥️ Devices & I/O
- [x] **Graphics:** Framebuffer output (Linear Graphics).
- [x] **Output:** 
    - Serial Port (COM1) logging.
    - Graphical Terminal (font8x8 rendering).
- [x] **Input:** 
    - PS/2 Keyboard (Async Stream).
    - PS/2 Mouse (Async Stream, Graphical Cursor).
- [x] **Filesystem:** 
    - InitRD (Ramdisk) via USTAR format.
    - Read-only support (`ls`, `cat`).

### 🛠️ Shell
- [x] Interactive Command Line Interface (CLI).
- [x] Commands: `help`, `ver`, `echo`, `ls`, `cat`, `clear`, `cpu`.

---

## 🗺️ Roadmap & Goals

### 🚧 Phase 2: Hypervisor (In Progress)
The main goal is to run Linux as a Guest VM.
- [x] **VMX Detection:** CPUID feature check.
- [x] **VMX Enable:** Executing `vmxon` instruction.
- [x] **VMCS Setup:** Configuring Virtual Machine Control Structure.
- [ ] **Guest State:** Setting up guest registers and segments.
- [ ] **VM Loop:** `vmlaunch` / `vmresume` implementation.
- [ ] **EPT:** Extended Page Tables (Memory virtualization).

### 🔮 Phase 3: User Mode & Security
- [ ] **Ring 3 Jump:** Context switching to User Mode.
- [ ] **Syscalls:** `syscall`/`sysret` handler.
- [ ] **ELF Loader:** Loading userspace programs.

### 🌟 Phase 4: Compatibility & GUI
- [ ] **Window Manager:** Drag & Drop windows.
- [ ] **Linux VM Integration:** Running Linux kernel inside RizOS.
- [ ] **PCIe Passthrough:** Passing GPU to the Linux Guest.
