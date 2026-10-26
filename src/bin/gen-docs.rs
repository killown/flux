//! Regenerates the derived documentation under `docs/`.
//!
//! Run via `cargo run --bin gen-docs`. Besides `docs/`, it also writes
//! `completions/` and `data/fish/`. CI runs it and fails if `git diff` or
//! `git status --porcelain` reports any change under those three paths, so
//! any drift between source and committed output is a build failure.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("docs")?;

    std::fs::write("docs/cli.md", flux::_doc_gen::cli::render_cli_markdown())?;
    std::fs::write(
        "docs/config.md",
        flux::_doc_gen::config::render_config_markdown(),
    )?;
    std::fs::write(
        "docs/css-variables.md",
        flux::_doc_gen::css_variables::render_css_variables_markdown(),
    )?;
    std::fs::write(
        "docs/actions.md",
        flux::_doc_gen::actions::render_actions_markdown(),
    )?;
    std::fs::write(
        "docs/search.md",
        flux::_doc_gen::search::render_search_markdown(),
    )?;
    std::fs::create_dir_all("completions")?;
    std::fs::write(
        "completions/flux-fm.fish",
        flux::_doc_gen::fish::render_completions(),
    )?;
    std::fs::create_dir_all("data/fish")?;
    std::fs::write(
        "data/fish/flux-integration.fish",
        flux::_doc_gen::fish::render_integration(),
    )?;

    println!("regenerated docs/cli.md");
    println!("regenerated docs/config.md");
    println!("regenerated docs/css-variables.md");
    println!("regenerated docs/actions.md");
    println!("regenerated docs/search.md");
    println!("regenerated completions/flux-fm.fish");
    println!("regenerated data/fish/flux-integration.fish");
    Ok(())
}
