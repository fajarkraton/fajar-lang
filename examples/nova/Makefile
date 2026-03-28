# FajarOS Nova v1.4.0 "Zenith" — Build and Run
#
# Usage:
#   make build     Build kernel binary from .fj source
#   make run       Build + boot in QEMU
#   make test      Build + run automated QEMU tests
#   make clean     Remove build artifacts
#
# Prerequisites:
#   - Fajar Lang compiler: cargo install --path .
#   - QEMU: sudo apt install qemu-system-x86
#   - xorriso: sudo apt install xorriso (for ISO creation)

KERNEL_FJ  := ../fajaros_nova_kernel.fj
KERNEL_BIN := fajaros_nova_kernel
ISO        := fajaros_nova.iso
FJ         := ../../target/release/fj

QEMU       := qemu-system-x86_64
QEMU_FLAGS := -serial stdio -display none -no-reboot -m 128M
NVME_DISK  := /tmp/nova_nvme.img

.PHONY: build run test clean fj

# Ensure fj compiler is built
fj:
	@if [ ! -f $(FJ) ]; then \
		echo "[make] Building Fajar Lang compiler (release)..."; \
		cd ../.. && cargo build --release; \
	fi

# Step 1: Check kernel source (type check)
check: fj
	@echo "[make] Type-checking kernel..."
	@$(FJ) check $(KERNEL_FJ)

# Step 2: Build kernel (native compile to flat binary)
# NOTE: Full native compilation of the 21K-line kernel requires
# the Cranelift bare-metal backend. For now, the kernel is
# pre-compiled as a flat binary (examples/fajaros_nova_kernel).
# When native compilation is ready:
#   $(FJ) build --target x86_64-none --no-std -o $(KERNEL_BIN) $(KERNEL_FJ)
build: check
	@echo "[make] Kernel binary: $(KERNEL_BIN)"
	@if [ -f ../$(KERNEL_BIN) ]; then \
		cp ../$(KERNEL_BIN) $(KERNEL_BIN); \
		echo "[make] Using pre-compiled kernel ($(shell wc -c < ../$(KERNEL_BIN)) bytes)"; \
	else \
		echo "[make] ERROR: No pre-compiled kernel found."; \
		echo "[make] Native compilation of 21K-line kernel not yet supported."; \
		exit 1; \
	fi

# Step 3: Create bootable ISO
iso: build
	@echo "[make] Creating bootable ISO..."
	@mkdir -p iso/boot/grub
	@cp $(KERNEL_BIN) iso/boot/kernel
	@echo 'menuentry "FajarOS Nova" { multiboot2 /boot/kernel }' > iso/boot/grub/grub.cfg
	@grub-mkrescue -o $(ISO) iso/ 2>/dev/null || \
		xorriso -as mkisofs -b boot/kernel -no-emul-boot -o $(ISO) iso/ 2>/dev/null || \
		echo "[make] WARNING: grub-mkrescue/xorriso not found — ISO creation skipped"
	@rm -rf iso/

# Step 4: Boot in QEMU
run: build
	@echo "[make] Booting FajarOS Nova in QEMU..."
	@# Create NVMe disk image if not present
	@if [ ! -f $(NVME_DISK) ]; then \
		qemu-img create -f raw $(NVME_DISK) 64M 2>/dev/null || true; \
	fi
	@$(QEMU) -kernel $(KERNEL_BIN) $(QEMU_FLAGS) \
		-drive file=$(NVME_DISK),if=none,id=nvme0,format=raw \
		-device nvme,serial=deadbeef,drive=nvme0 \
		-net nic,model=virtio -net user

# Step 5: Automated tests
test: build
	@echo "[make] Running QEMU tests..."
	@cd .. && bash fajaros_nova_test.sh $(KERNEL_BIN)

# Clean
clean:
	@rm -f $(KERNEL_BIN) $(ISO)
	@rm -rf iso/
	@echo "[make] Clean."
