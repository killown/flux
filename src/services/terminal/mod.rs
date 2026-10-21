mod cwd;
mod draw;
mod fish_theme;
mod handler;
mod palette;
mod selection;
mod spawn;
mod state;
mod theme;
mod widget;

#[allow(unused_imports)]
pub(crate) use draw::SCROLLBAR_WIDTH;
#[allow(unused_imports)]
pub use handler::TerminalHandler;
#[allow(unused_imports)]
pub use state::{Cell, CursorStyle, TerminalState};
pub(crate) use theme::ThemeEnv;
#[allow(unused_imports)]
pub(crate) use cwd::{setup_goto_watcher, write_cwd_cache};
#[allow(unused_imports)]
pub(crate) use selection::{print_selection, setup_selection_server};

use crate::model::TerminalConfig;
use gtk::DrawingArea;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;

pub struct Terminal {
    pub config: TerminalConfig,
    pub drawing_area: DrawingArea,
    pub state: Arc<Mutex<TerminalState>>,
    _pty_reader: Option<std::thread::JoinHandle<()>>,
    /// Shared dirty flag written by the PTY reader thread and polled by the
    /// 60 FPS GTK timer.
    needs_redraw: Arc<AtomicBool>,
    /// Last directory queued for respawn, only the most recent survives rapid navigation.
    pending_dir: Arc<Mutex<Option<String>>>,
    /// Resolved theme colours, populated by `apply_theme` after first map.
    pub(crate) theme_env: Arc<Mutex<Option<ThemeEnv>>>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("drawing_area", &self.drawing_area)
            .finish()
    }
}

impl Clone for Terminal {
    fn clone(&self) -> Self {
        Self {
            theme_env: self.theme_env.clone(),
            drawing_area: self.drawing_area.clone(),
            state: self.state.clone(),
            config: self.config.clone(),
            _pty_reader: None,
            needs_redraw: self.needs_redraw.clone(),
            pending_dir: self.pending_dir.clone(),
        }
    }
}
