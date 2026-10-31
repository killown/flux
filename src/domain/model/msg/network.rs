//! Network locations and bookmarks.

use crate::domain::model::{NetworkAuthFlags, NetworkCredentials};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NetworkMsg {
    /// Toggles the right connect-to-server panel (Ctrl+Shift+L).
    ToggleConnectPanel,
    /// Delivers the result of an async network directory listing.
    ///
    /// Analogous to `ArchiveLoaded`, carries the pre-computed load contexts so
    /// the GTK main thread can populate the grid without touching GIO directly.
    NetworkLoaded {
        uri: String,
        contexts: Vec<crate::domain::model::FileLoadContext>,
    },
    /// Navigate to the `network:///` GIO location that lists visible network neighbours.
    NavigateNetwork,

    /// Connect to a remote server using the supplied connection parameters.
    ///
    /// Dispatched by the "Connect to Server" dialog. The URI is built from
    /// `infra::network::ConnectToServerParams::build_uri` and then
    /// handled identically to `Navigate(PathBuf::from(uri))`.
    ConnectToServer {
        /// The canonical GIO URI of the remote location (e.g. `smb://server/share`).
        uri: String,
        /// Optional credentials to pre-populate the GVFS mount operation.
        credentials: Option<NetworkCredentials>,
    },

    /// The remote server at `uri` requires credentials before the listing can proceed.
    ///
    /// Dispatched by `load_network` when `infra::network::NetworkError::CredentialsRequired`
    /// is returned. The update loop shows the GTK credentials dialog and, on
    /// confirmation, re-dispatches [`NetworkMsg::ConnectToServer`] with the filled credentials.
    PromptNetworkCredentials {
        uri: String,
        /// Human-readable prompt from the GVFS backend.
        message: String,
        /// Which fields the server needs.
        flags: NetworkAuthFlags,
        /// `true` when a previous attempt was rejected - the dialog shows an error hint.
        auth_failed: bool,
    },

    /// Persist a network location as a sidebar bookmark.
    AddNetworkBookmark { name: String, uri: String },

    /// Remove a network bookmark by URI.
    RemoveNetworkBookmark(String),

    /// Unmount the GVFS mount backing the given network URI.
    UnmountNetwork(String),

    /// Rebuild the "Network" section in the sidebar from the current active GVFS mounts.
    RefreshNetworkSidebar,
}
