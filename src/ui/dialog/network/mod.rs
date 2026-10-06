//! GTK4/Libadwaita dialogs for network connection and credential entry.
//!
//! Provides two dialogs:
//! - [`show_connect_to_server`] - collects protocol, host, port, path, and username,
//!   then dispatches [`crate::model::AppMsg::ConnectToServer`].
//! - [`show_credentials_dialog`] - prompts for a username and password when GVFS
//!   reports that a remote location requires authentication.
//!
//! Both dialogs are non-blocking modal windows that dispatch an [`AppMsg`] on
//! confirmation, keeping the GTK event loop free.

mod connect;
mod credentials;

pub use connect::register_connect_action;
pub use credentials::show_credentials_dialog;
