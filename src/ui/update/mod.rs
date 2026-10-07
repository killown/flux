use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

mod context_menu;
mod dialogs;
mod file_ops;
mod git;
mod loading;
mod navigation;
mod network;
mod search;
mod sidebar;
mod thumbnails;
mod view;
mod window;

impl FluxApp {
    pub fn handle_update(&mut self, mut message: AppMsg, sender: AsyncComponentSender<Self>) {
        #[cfg(debug_assertions)]
        let _slow = {
            use std::fmt::Write as _;
            let mut head = Head(String::new());
            let _ = write!(head, "{message:?}");
            SlowMsg {
                start: std::time::Instant::now(),
                name: head.0,
            }
        };
        crate::hit!("handle_update");

        macro_rules! route {
            ($($m:ident),+ $(,)?) => {
                $(
                    match $m::handle(self, message, &sender) {
                        Ok(()) => return,
                        Err(rest) => message = rest,
                    }
                )+
            };
        }

        route!(
            navigation,
            loading,
            view,
            thumbnails,
            search,
            file_ops,
            dialogs,
            context_menu,
            network,
            git,
            window,
            sidebar,
        );

        eprintln!("[flux] unhandled AppMsg: {message:?}");
    }
}

// ── Slow-message instrumentation (unchanged) ────────────────────────────

#[cfg(debug_assertions)]
struct Head(String);
#[cfg(debug_assertions)]
impl std::fmt::Write for Head {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let room = 80usize.saturating_sub(self.0.len());
        self.0.extend(s.chars().take(room));
        if self.0.len() >= 80 {
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}
#[cfg(debug_assertions)]
struct SlowMsg {
    start: std::time::Instant,
    name: String,
}
#[cfg(debug_assertions)]
impl Drop for SlowMsg {
    fn drop(&mut self) {
        if std::env::var_os("FLUX_HWGA_OUT").is_none() {
            return;
        }
        let took = self.start.elapsed();
        if took > std::time::Duration::from_millis(32) {
            eprintln!("[slow AppMsg] {took:?} {}", self.name);
        }
    }
}
