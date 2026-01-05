#!/bin/bash
set -e

LIMINE_PATH="./bootloader"

echo "=== 1. Building Kernel ==="
# Собираем ядро (LTO должно быть false в Cargo.toml)
cargo build --release --package kernel

echo "=== 2. Creating ISO Structure ==="
rm -rf iso_root
mkdir -p iso_root/boot/limine
mkdir -p iso_root/EFI/BOOT

# --- НОВАЯ ЧАСТЬ: СОЗДАНИЕ ДИСКА ---
echo "Packing files..."
# Создаем архив disk.tar из содержимого папки files/
# Флаг --format=ustar важен для совместимости!
tar -cvf disk.tar -C files/ --format=ustar .

# Копируем архив в ISO
cp disk.tar iso_root/boot/disk.tar
# -----------------------------------

# Копируем ядро
cp target/x86_64-unknown-none/release/kernel iso_root/boot/

# Копируем НОВЫЙ конфиг в /boot/limine/limine.conf
cp limine.conf iso_root/boot/limine/limine.conf

# Копируем файлы загрузчика
cp "$LIMINE_PATH/limine-bios.sys" iso_root/boot/limine/
cp "$LIMINE_PATH/limine-bios-cd.bin" iso_root/boot/limine/
cp "$LIMINE_PATH/limine-uefi-cd.bin" iso_root/boot/limine/
cp "$LIMINE_PATH/limine-uefi-cd.bin" iso_root/

echo "=== 3. Burning ISO ==="
xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
        -no-emul-boot -boot-load-size 4 -boot-info-table \
        --efi-boot limine-uefi-cd.bin \
        -efi-boot-part --efi-boot-image --protective-msdos-label \
        iso_root -o riz_os.iso

echo "=== 4. Installing BIOS Bootloader ==="
"$LIMINE_PATH/limine" bios-install riz_os.iso

echo "=== 5. Running QEMU ==="
OVMF_PATH="/usr/share/edk2/ovmf/OVMF_CODE.fd"
[ ! -f "$OVMF_PATH" ] && OVMF_PATH="/usr/share/OVMF/OVMF_CODE.fd"

# Запускаем!
qemu-system-x86_64 -enable-kvm -cpu host -M q35 -m 2G -cdrom riz_os.iso -boot d -serial stdio -bios "$OVMF_PATH" -vga std