//! A simple `no_std` heap allocator for RISC-V and Xtensa processors from
//! Espressif. Supports all currently available ESP32 devices.
//!
//! **NOTE:** using this as your global allocator requires using Rust 1.68 or
//! greater, or the `nightly` release channel.

#![no_std]

pub mod macros;

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
    ptr::{self, NonNull},
};

use critical_section::Mutex;
use linked_list_allocator::Heap;

pub struct EspHeap {
    heap: Mutex<RefCell<Heap>>,
    heap2: Mutex<RefCell<Heap>>,
}

impl EspHeap {
    /// Crate a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](struct.EspHeap.html#method.init) method before using the
    /// allocator.
    pub const fn empty() -> EspHeap {
        EspHeap {
            heap: Mutex::new(RefCell::new(Heap::empty())),
            heap2: Mutex::new(RefCell::new(Heap::empty())),
        }
    }

    /// Initializes the heap
    ///
    /// This function must be called BEFORE you run any code that makes use of
    /// the allocator.
    ///
    /// `heap_bottom` is a pointer to the location of the bottom of the heap.
    ///
    /// `size` is the size of the heap in bytes.
    ///
    /// Note that:
    ///
    /// - The heap grows "upwards", towards larger addresses. Thus `end_addr`
    ///   must be larger than `start_addr`
    ///
    /// - The size of the heap is `(end_addr as usize) - (start_addr as usize)`.
    ///   The allocator won't use the byte at `end_addr`.
    ///
    /// # Safety
    ///
    /// Obey these or Bad Stuff will happen.
    ///
    /// - This function must be called exactly ONCE.
    /// - `size > 0`
    pub unsafe fn init(&self, heap_bottom: *mut u8, size: usize) {
        critical_section::with(|cs| self.heap.borrow(cs).borrow_mut().init(heap_bottom, size));
    }

    /// Initializes optional second heap area, mainly to utilize 2nd incontiguous ram space
    ///
    /// This function may be called BEFORE you run any code that makes use of
    /// the allocator.
    ///
    /// `heap_bottom` is a pointer to the location of the bottom of the heap.
    ///
    /// `size` is the size of the heap in bytes.
    ///
    /// Note that:
    ///
    /// - The heap grows "upwards", towards larger addresses. Thus `end_addr`
    ///   must be larger than `start_addr`
    ///
    /// - The size of the heap is `(end_addr as usize) - (start_addr as usize)`.
    ///   The allocator won't use the byte at `end_addr`.
    ///
    /// # Safety
    ///
    /// Obey these or Bad Stuff will happen.
    ///
    /// - This function must be called exactly ONCE.
    /// - `size > 0`
    pub unsafe fn init_heap2(&self, heap_bottom: *mut u8, size: usize) {
        critical_section::with(|cs| self.heap2.borrow(cs).borrow_mut().init(heap_bottom, size));
    }

    /// Returns an estimate of the amount of bytes in use.
    pub fn used(&self) -> usize {
        let mut used = critical_section::with(|cs| self.heap.borrow(cs).borrow_mut().used());
        used += critical_section::with(|cs| self.heap2.borrow(cs).borrow_mut().used());
        used
    }

    /// Returns an estimate of the amount of bytes available.
    pub fn free(&self) -> usize {
        let mut free = critical_section::with(|cs| self.heap.borrow(cs).borrow_mut().free());
        free += critical_section::with(|cs| self.heap2.borrow(cs).borrow_mut().free());
        free 
    }
}

unsafe impl GlobalAlloc for EspHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut ptr = critical_section::with(|cs| {
            self.heap
                .borrow(cs)
                .borrow_mut()
                .allocate_first_fit(layout)
                .ok()
                .map_or(ptr::null_mut(), |allocation| allocation.as_ptr())
        });

        if ptr == ptr::null_mut() { 
            ptr = critical_section::with(|cs| {
                self.heap2
                    .borrow(cs)
                    .borrow_mut()
                    .allocate_first_fit(layout)
                    .ok()
                    .map_or(ptr::null_mut(), |allocation| allocation.as_ptr())
            });
        };

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let done = critical_section::with(|cs| {
            let mut borrowed_heap = self.heap.borrow(cs).borrow_mut();
            if ptr >= borrowed_heap.bottom() && ptr <= borrowed_heap.top() {
                borrowed_heap.deallocate(NonNull::new_unchecked(ptr), layout);
                return true;
            };
            return false;
        });
        if !done {
            critical_section::with(|cs| {
                let mut borrowed_heap2 = self.heap2.borrow(cs).borrow_mut();
                    borrowed_heap2.deallocate(NonNull::new_unchecked(ptr), layout);
            });
        }
    }
}
