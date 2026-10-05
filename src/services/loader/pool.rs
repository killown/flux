use std::sync::OnceLock;

/// Shared Rayon thread pool for directory listing work.
///
/// Capped at 4 threads with 2 MiB stacks instead of Rayon's global default
/// (one thread per CPU core, 8 MiB stacks each). This prevents the one-time
/// ~50 MiB RSS jump that occurs when the global pool fully commits its stacks
/// on the first large directory load.
static LOADER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

pub(super) fn loader_pool() -> &'static rayon::ThreadPool {
    LOADER_POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().clamp(4, 8))
            .unwrap_or(4);

        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .stack_size(2 * 1024 * 1024)
            .build()
            .expect("failed to build loader thread pool")
    })
}
