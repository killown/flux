mod app;
mod ui_components;
mod utils;
mod model;

use relm4::prelude::*;
use crate::model::FluxApp;
use std::path::PathBuf;

fn main() {
    let app = RelmApp::new("sh.flux.FileManager");
    let args: Vec<String> = std::env::args().collect();
    let start_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        dirs::home_dir().unwrap_or(PathBuf::from("."))
    };

    app.with_args(vec![])
       .run::<FluxApp>(start_path);
}
