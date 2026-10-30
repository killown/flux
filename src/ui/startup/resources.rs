use crate::domain::model::{Config, CustomAction, FluxApp};
use crate::infra::db::StateManager;
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
            // A corrupt, locked or unwritable state DB must not stop flux from starting.
            // The file is left untouched; history, tags and layouts just won't be saved.
            let db = crate::infra::db::StateManager::new().unwrap_or_else(|e| {
                eprintln!(
                    "flux: could not open the state database ({e}); \
                     using a temporary in-memory one, nothing will be saved this session"
                );
                crate::infra::db::StateManager::new_in_memory()
                    .expect("in-memory SQLite database must always open")
            });
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
