use lv_bevy_ecs::sys::lv_mem_monitor_t;
use static_cell::StaticCell;

const BSS_HEAP_SIZE: usize = 160 * 1024;
static BSS_HEAP: StaticCell<[u8; BSS_HEAP_SIZE]> = StaticCell::new();

const SRAM1_START: usize = 0x3FFE_8001;
const SRAM1_END: usize = 0x4000_0000;

#[allow(static_mut_refs)]
pub fn setup_heap() {
    unsafe {
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            BSS_HEAP.uninit().as_mut_ptr().cast(),
            BSS_HEAP_SIZE,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }

    unsafe {
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            SRAM1_START as *mut u8,
            SRAM1_END - SRAM1_START,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }

    defmt::info!("{}", esp_alloc::HEAP.stats());
}

#[allow(static_mut_refs)]
pub fn get_memory_stats(monitor: &mut lv_mem_monitor_t) {
    unsafe {
        static mut MAX_USED: usize = 0;
        let heap = &esp_alloc::HEAP;
        let stats = heap.stats();
        let used = stats.current_usage;
        let total = stats.size;
        monitor.free_size = heap.free();
        monitor.total_size = total;
        monitor.used_pct = (used * 100 / total) as u8;
        let max_used = usize::max(MAX_USED, used);
        monitor.max_used = max_used;
        MAX_USED = max_used;
    }
}
