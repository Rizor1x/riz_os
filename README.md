# RizOS 🦀

Experimental x86_64 Operating System written in Rust.
Features a custom GUI, Async/Await multitasking, and a native Type-1 Hypervisor (Intel VT-x).

## 🚀 Current Status: v0.2 (GUI & Hypervisor Base)

### 🎨 Graphical User Interface (GUI)
- [x] **Video:** Linear Framebuffer (1280x800).
- [x] **Engine:** Double Buffering (Zero flicker, no artifacts).
- [x] **Window System:**
    - Window rendering (Body, Title bar, Shadows).
    - **Drag & Drop** support (Move windows with mouse).
    - Content clipping (Text moves with window).
- [x] **Input:**
    - PS/2 Mouse (Hardware acceleration, smooth movement).
    - PS/2 Keyboard (Interrupt-based, responsive).

### ⚡ Hypervisor (Intel VT-x)
- [x] **Hardware Check:** VMX support detection.
- [x] **Lifecycle:** `VMXON`, `VMCLEAR`, `VMPTRLD` sequences implemented.
- [x] **Execution Loop:**
    - Custom Assembly trampoline for `vmlaunch`/`vmresume`.
    - Stable VM Exit handling (Host <-> Guest switching).
    - Interrupt Injection (Mouse works while VM is running).
- [x] **Guest State:** 64-bit "Mirror" Guest (Unrestricted Mode preparation).

### 🛠️ Core Features
- [x] **Filesystem:** InitRD (TAR) read-only support.
- [x] **Shell:** Interactive command line inside a GUI window.
- [x] **Multitasking:** Cooperative Async Executor + Hardware Interrupts.

---

## 🗺️ Roadmap: The Path to v0.3

### 🏗️ Phase 3: Window Manager (Refactoring)
Currently, window logic is hardcoded in `main.rs`.
- [ ] **WindowManager Class:** Abstract window creation (`WindowManager::new_window()`).
- [ ] **Multiple Windows:** Support for overlapping windows (Z-order).
- [ ] **Focus:** Click to focus / bring to front.
- [ ] **Widgets:** Buttons, Labels.

### 🐧 Phase 4: Running Linux (The Big Goal)
- [ ] **Guest Payload:** Loading a real kernel binary into Guest Memory (instead of `hlt` loop).
- [ ] **EPT (Extended Page Tables):** Implementing memory virtualization (isolating Guest RAM).
- [ ] **Serial Emulation:** intercepting Guest IO to show Linux boot logs in RizOS Shell.
