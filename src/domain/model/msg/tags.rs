//! File tags and the tag panel.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TagsMsg {
    /// Persists and updates the tag panel width in pixels.
    SetTagPanelWidth(i32),
    /// Persists the provided list of tags to both the database index and extended attributes
    /// (`user.xdg.tags`) for all items currently selected in the file view.
    ApplyTagsToSelection(Vec<String>),
    /// Removes a specific tag from all currently selected files and persists the updated metadata.
    RemoveTagFromSelection(String),
    /// Toggles the visibility of the lazy-initialized right tag navigator sidebar panel.
    ToggleTagPanel,
    /// Delete a tag from the database entirely across all files.
    DeleteTagGlobally(String),
    /// Open the tag picker popover for the current selection (Ctrl+T).
    OpenTagPicker,
    /// Deliver the resolved tags for the picker to display.
    TagsReady {
        paths: Vec<PathBuf>,
        tags: Vec<String>,
        available_tags: Vec<String>,
    },
    /// Persist a tag set to xattr + SQLite for the given path.
    SetFileTags { path: PathBuf, tags: Vec<String> },
    /// Navigate to a virtual tag view or filter by tag.
    NavigateTag(String),
    /// Open the "Save search as tag" dialog for the results the active tab shows.
    PromptSaveSearchAsTag,
    /// Add `tag` to the tags of every path in `paths`, keeping the tags they already have,
    /// then report back with [`TagsMsg::SaveSearchAsTagDone`].
    SaveSearchAsTag {
        tag: String,
        paths: Vec<PathBuf>,
        pin_to_sidebar: bool,
    },
    /// Outcome of [`TagsMsg::SaveSearchAsTag`]: `count` files were tagged, `failed` were not.
    SaveSearchAsTagDone {
        tag: String,
        count: usize,
        failed: usize,
        pin_to_sidebar: bool,
    },
}
