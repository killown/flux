#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use vte::Parser;

fuzz_target!(|data: &[u8]| {
    let state = Arc::new(Mutex::new(flux::services::terminal::TerminalState::new(
        80, 24,
    )));
    let needs_redraw = Arc::new(AtomicBool::new(false));
    let mut handler = flux::services::terminal::TerminalHandler {
        state,
        needs_redraw,
    };
    let mut parser = Parser::new();
    parser.advance(&mut handler, data);
});
