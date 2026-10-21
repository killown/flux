//! Regenerates the derived documentation under `docs/`.
//!
//! Run via `cargo run --bin gen-docs`. CI runs it and checks
//! `git diff --exit-code docs/` so any drift between source and committed
//! markdown is a build failure.

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

    println!("regenerated docs/cli.md");
    println!("regenerated docs/config.md");
    println!("regenerated docs/css-variables.md");
    println!("regenerated docs/actions.md");
    println!("regenerated docs/search.md");
    Ok(())
}
