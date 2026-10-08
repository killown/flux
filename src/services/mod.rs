pub mod archive;
pub mod db;
pub mod git;
pub mod inspector;
pub mod loader;
pub mod luks;
pub mod mounts;
pub mod network;
pub mod search;
pub mod tasks;
pub mod terminal;
pub mod thumbnails;
pub mod trash;

pub mod constants {
    pub const MAX_CONTENT_SEARCH_RESULTS: usize = 100;
}
