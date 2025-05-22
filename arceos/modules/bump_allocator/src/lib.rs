#![no_std]
extern crate alloc;

use core::alloc::Layout;
use core::ptr::NonNull;
use core::result;
use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use log::info;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}
impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }

    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;

        info!(
            "[early_alloc] init: [{:#x}, {:#x}), total = {} KB",
            start,
            self.end,
            size / 1024
        );
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        Ok(())
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();

        let aligned = Self::align_up(self.b_pos, align);
        let new_b_pos = aligned.checked_add(size).ok_or(AllocError::InvalidParam)?;

        if new_b_pos > self.p_pos {
            return Err(AllocError::NoMemory);
        }

        self.b_pos = new_b_pos;
        self.count += 1;

        Ok(unsafe { NonNull::new_unchecked(aligned as *mut u8) })
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        assert!(self.count > 0);
        self.count -= 1;
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        let size = num_pages * PAGE_SIZE;
        let align = 1 << align_pow2;

        let mut new_p_pos = self.p_pos.checked_sub(size).ok_or(AllocError::InvalidParam)?;
        new_p_pos &= !(align - 1);

        if new_p_pos < self.b_pos {
            return Err(AllocError::NoMemory);
        }

        self.p_pos = new_p_pos;
        Ok(self.p_pos)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        self.available_bytes() / PAGE_SIZE
    }
}