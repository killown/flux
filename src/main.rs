mod app;
mod ui_components;

use relm4::prelude::*;
use crate::app::FluxApp;

fn main() {
    let app = RelmApp::new("sh.flux.FileManager");
    app.run::<FluxApp>(());
}
