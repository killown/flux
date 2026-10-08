#[derive(Debug, Clone, Default)]
pub(super) struct MallinfoSnap {
    pub arena_bytes: usize,
    pub ordblks: usize,
    pub uordblks_bytes: usize,
    pub fordblks_bytes: usize,
    pub hblkhd_bytes: usize,
}

pub(super) fn read_mallinfo() -> MallinfoSnap {
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct StructMallinfo2 {
            arena: usize,
            ordblks: usize,
            smblks: usize,
            hblks: usize,
            hblkhd: usize,
            usmblks: usize,
            fsmblks: usize,
            uordblks: usize,
            fordblks: usize,
            keepcost: usize,
        }

        extern "C" {
            fn mallinfo2() -> StructMallinfo2;
        }

        let raw_info = unsafe { mallinfo2() };
        MallinfoSnap {
            arena_bytes: raw_info.arena,
            ordblks: raw_info.ordblks,
            uordblks_bytes: raw_info.uordblks,
            fordblks_bytes: raw_info.fordblks,
            hblkhd_bytes: raw_info.hblkhd,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        MallinfoSnap::default()
    }
}
