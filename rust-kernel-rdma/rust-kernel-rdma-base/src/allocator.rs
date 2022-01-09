use core::alloc::{Layout, AllocRef, AllocError};
use crate::rust_kernel_linux_util::bindings;
use crate::linux_kernel_module::c_types;
use core::ptr::NonNull;

pub struct VmallocAllocator;

unsafe impl AllocRef for VmallocAllocator {
    fn alloc(&self, layout: Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        match layout.size() {
            0 => Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0)),
            // SAFETY: `layout` is non-zero in size,
            size => {
                let raw_ptr = unsafe { bindings::vmalloc(size as u64) } as *mut u8;
                let ptr = NonNull::new(raw_ptr).ok_or(AllocError)?;
                Ok(NonNull::slice_from_raw_parts(ptr, size))
            }
        }
    }

    unsafe fn dealloc(&self, ptr: NonNull<u8>, _layout: Layout) {
        bindings::vfree(ptr.as_ptr() as *const c_types::c_void);
    }
}
