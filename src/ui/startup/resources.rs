use crate::model::{Config, CustomAction, FluxApp};
use crate::services::db::StateManager;
use crate::utils;
use futures::try_join;

impl FluxApp {
    /// Loads the state DB, config and menu actions concurrently.
    pub(super) async fn load_resources() -> (StateManager, Config, Vec<CustomAction>) {
        // Resource Loading (asynchronous, parallel)
        let (config_tx, config_rx) = tokio::sync::oneshot::channel();
        let (menu_tx, menu_rx) = tokio::sync::oneshot::channel();
        let (db_tx, db_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let db = crate::services::db::StateManager::new().expect("DB Init Failed");
            let _ = db_tx.send(db);
        });
        tokio::spawn(async move {
            let cfg = utils::load_config();
            let _ = config_tx.send(cfg);
        });
        tokio::spawn(async move {
            let menu = utils::load_menu_config();
            let _ = menu_tx.send(menu);
        });

        // Wait for all to complete concurrently
        let (state_db_res, config, menu_actions_list) = {
            crate::hit!("init_components:resources");
            try_join!(db_rx, config_rx, menu_rx)
                .expect("Initialization tasks should always complete")
        };
        (state_db_res, config, menu_actions_list)
    }
}
