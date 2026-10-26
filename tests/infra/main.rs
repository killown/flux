//! Tests for `flux::infra` (archives, database, loader, network, LUKS, ...).

mod archive {
    mod backend;
    mod extract_prefix;
    mod extract_tar_links;
    mod extract_traversal;
    mod synthesis;
    mod uri_and_listing;
    mod zip_encrypted_write;
}

mod db {
    mod advanced;
    mod quick_groups;
    mod quick_list;
    mod state_manager;
}

// `delete_ops` (no tests at all, dead code), `loader::sorting`, `search::ops`
// and `thumbnails` re-implement the logic they assert on instead of calling
// into `flux`.
mod delete_ops;
mod keyring;

mod loader {
    mod directory;
    mod sorting;
}

mod luks {
    mod image;
    mod parse;
    mod unlock;
}

mod mounts;
mod network;

mod search {
    mod indexer;
    mod ops;
}

mod tasks;
mod terminal;
mod thumbnails;
mod trash;
