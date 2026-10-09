use crate::model::{Config, CustomAction, FluxApp};
use crate::services::db::StateManager;
use crate::utils;
use futures::try_join;

/// State DB that is still opening on a worker thread.
pub(super) struct PendingDb(tokio::sync::oneshot::Receiver<StateManager>);

impl PendingDb {
    /// Waits for the DB to finish opening.
    pub(super) async fn wait(self) -> StateManager {
        crate::hit!("init_components:db_wait");
        self.0
            .await
            .expect("Initialization tasks should always complete")
    }
}

impl FluxApp {
    /// Loads config and menu actions, and starts opening the state DB in the background.
    pub(super) async fn load_resources() -> (PendingDb, Config, Vec<CustomAction>) {
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

        // Only config and menu are awaited here, the DB keeps opening while the UI is built.
        let (config, menu_actions_list) = {
            crate::hit!("init_components:resources");
            try_join!(config_rx, menu_rx).expect("Initialization tasks should always complete")
        };
        (PendingDb(db_rx), config, menu_actions_list)
    }
}
