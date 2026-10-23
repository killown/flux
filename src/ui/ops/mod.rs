pub(crate) mod view;

#[allow(unused_imports)]
pub use crate::app::update::app_action;
#[allow(unused_imports)]
pub use crate::app::update::file_ops as file;
pub use crate::app::update::file_ops::paste;
#[allow(unused_imports)]
pub use crate::app::update::git;
#[allow(unused_imports)]
pub use crate::app::update::network as remote;
#[allow(unused_imports)]
pub use crate::app::update::sidebar;
#[allow(unused_imports)]
pub use crate::app::update::tag;
#[allow(unused_imports)]
pub use crate::app::update::task;
#[allow(unused_imports)]
pub use crate::app::update::terminal;
#[allow(unused_imports)]
pub use crate::app::update::toast;
#[allow(unused_imports)]
pub use crate::app::update::undo;
#[allow(unused_imports)]
pub use crate::app::update::watcher;

pub(crate) use crate::app::update::watcher::debounce_path_event;
