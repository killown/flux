// FIXME: Right-click does not select the item on SMB/network shares.
// The path retrieval via widget.data() or widget.widget_name() fails
// due to GTK widget recycling in the factory. Even with Rc<RefCell<PathBuf>>
// stored in the model and updated in bind(), the gesture closure sometimes
// reads a stale/None value when navigating rapidly or on first load.
// Possible causes: async loading triggers unbind/rebind before the gesture
// fires, or the gesture target is not the root widget where the data is stored.
// Workaround attempts: using widget name fallback, removing duplicate handler
// in grid_scroller, forcing grid refresh. Still not reliable for all cases.
// Consider rewriting the right‑click selection logic to use the selection model
// directly or pass the index via the gesture closure (capture index at bind time).

#![allow(dead_code)]

mod debug;

mod bookmark;
mod browse;
mod credentials;
mod entry;
mod error;
mod location;
mod mount;
mod protocol;

pub use bookmark::NetworkBookmark;
pub use browse::{create_network_directory, list_network_entries};
pub use credentials::NetworkCredentials;
pub use entry::entries_to_load_contexts;
pub use error::{NetworkAuthFlags, NetworkError};
pub use location::ConnectToServerParams;
pub use mount::{active_mounts, unmount_network_location};
pub use protocol::*;

// Part of the module's public API, but not referenced outside it yet.
#[allow(unused_imports)]
pub use entry::NetworkEntry;
#[allow(unused_imports)]
pub use location::describe_network_location;
#[allow(unused_imports)]
pub use mount::check_gvfs_deps;
