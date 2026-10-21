mod actions;
mod components;
mod deferred;
mod grid;
mod menu;
mod resources;
mod sidebar;
mod tabs;
mod terminal;

pub(crate) use terminal::{
    attach_terminal_key_controller, attach_terminal_mouse_controllers, register_terminal_shutdown,
    wire_terminal_theme,
};
