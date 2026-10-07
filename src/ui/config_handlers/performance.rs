use crate::model::FluxApp;
use crate::utils;

impl FluxApp {
    pub fn handle_set_loader_batch_size(&mut self, val: usize) {
        self.config.ui.loader_batch_size = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_folder_cache_capacity(&mut self, val: usize) {
        self.config.ui.folder_cache_capacity = val;
        while self.folder_cache.len() > val && val > 0 {
            if let Some(oldest) = self
                .folder_cache
                .iter()
                .min_by_key(|(_, v)| v.last_visited)
                .map(|(k, _)| k.clone())
            {
                self.folder_cache.remove(&oldest);
            }
        }
        if val == 0 {
            self.folder_cache.clear();
        }
        unsafe {
            libc::malloc_trim(0);
        }
        utils::save_config(&self.config);
    }

    pub fn handle_set_max_history(&mut self, val: usize) {
        self.config.ui.max_history = val;
        if self.history.len() > val {
            let truncate_count = self.history.len() - val;
            self.history.drain(0..truncate_count);
        }
        utils::save_config(&self.config);
    }
}
