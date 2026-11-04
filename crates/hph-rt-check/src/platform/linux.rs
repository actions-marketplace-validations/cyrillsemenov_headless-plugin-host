#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct mallinfo2 {
    pub arena: usize,
    pub ordblks: usize,
    pub smblks: usize,
    pub hblks: usize,
    pub hblkhd: usize,
    pub usmblks: usize,
    pub fsmblks: usize,
    pub uordblks: usize,
    pub fordblks: usize,
    pub keepcost: usize,
}

extern "C" {
    fn mallinfo2() -> mallinfo2;
}

#[inline]
pub fn snapshot_process_heap() -> mallinfo2 {
    unsafe { mallinfo2() }
}
