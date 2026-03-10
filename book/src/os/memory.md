# Memory Management

## Physical Memory

```fajar
@kernel fn setup() {
    let ptr = mem_alloc(4096)           // allocate 4KB
    mem_write(ptr, 0, 0xDEADBEEF)      // write at offset 0
    let val = mem_read(ptr, 0)          // read back
    mem_free(ptr)                        // deallocate
}
```

## Page Tables

```fajar
@kernel fn map_video() {
    page_map(0xB8000, PageFlags::ReadWrite)
    page_map(0xB9000, PageFlags::ReadOnly)
    page_unmap(0xB8000)
}
```

## Memory Operations

```fajar
@kernel fn copy_data() {
    memory_copy(dest, src, 256)          // memcpy
    memory_set(buffer, 0, 4096)          // memset
    let eq = memory_compare(a, b, 16)    // memcmp
}
```

## Custom Allocators

Fajar includes three built-in allocator strategies:

```fajar
// Bump allocator (fast, no dealloc)
let bump = BumpAllocator::new(heap_start, heap_size)

// Free-list allocator (general purpose)
let freelist = FreeListAllocator::new(heap_start, heap_size)

// Pool allocator (fixed-size blocks)
let pool = PoolAllocator::new(heap_start, block_size, count)
```

## Type-Safe Addresses

```fajar
@kernel fn safe_addressing() {
    let phys: PhysAddr = PhysAddr(0x1000)
    let virt: VirtAddr = VirtAddr(0xFFFF_8000_0000_0000)
    // PhysAddr and VirtAddr are distinct types -- cannot be mixed
}
```
