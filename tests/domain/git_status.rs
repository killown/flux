//! `GitFileStatus::from_porcelain`: `git status --porcelain=v1` codes to the
//! status the grid decorates a file with.

use flux::domain::model::GitFileStatus as Status;

fn parse(code: &str) -> Status {
    let bytes = code.as_bytes();
    assert_eq!(
        bytes.len(),
        2,
        "porcelain codes are two characters: {code:?}"
    );
    Status::from_porcelain(bytes[0], bytes[1])
}

#[test]
fn untracked_and_ignored_files() {
    assert_eq!(parse("??"), Status::Untracked);
    assert_eq!(parse("!!"), Status::Ignored);
}

#[test]
fn changes_that_are_only_in_the_index() {
    assert_eq!(parse("A "), Status::Added);
    assert_eq!(parse("M "), Status::StagedModified);
    assert_eq!(parse("R "), Status::Renamed);
    assert_eq!(parse("T "), Status::TypeChanged);
    assert_eq!(parse("D "), Status::Deleted);
}

#[test]
fn changes_that_are_only_in_the_working_tree() {
    assert_eq!(parse(" M"), Status::Modified);
    assert_eq!(parse(" T"), Status::TypeChanged);
    assert_eq!(parse(" D"), Status::Deleted);
}

#[test]
fn a_file_changed_in_both_places_is_mixed() {
    assert_eq!(parse("MM"), Status::MixedModified);
    assert_eq!(parse("AM"), Status::MixedModified);
}

#[test]
fn a_deletion_on_either_side_wins_over_other_changes() {
    assert_eq!(parse("MD"), Status::Deleted);
    assert_eq!(parse("AD"), Status::Deleted);
    assert_eq!(parse("RD"), Status::Deleted);
}

#[test]
fn a_rename_with_later_edits_is_still_a_rename() {
    assert_eq!(parse("RM"), Status::Renamed);
}

#[test]
fn every_unmerged_combination_is_a_conflict() {
    for code in ["DD", "AU", "UD", "UA", "DU", "AA", "UU"] {
        assert_eq!(parse(code), Status::Conflicted, "{code}");
    }
}

#[test]
fn an_unmerged_marker_on_either_side_is_a_conflict() {
    for other in b" MTADRC?!" {
        assert_eq!(Status::from_porcelain(b'U', *other), Status::Conflicted);
        assert_eq!(Status::from_porcelain(*other, b'U'), Status::Conflicted);
    }
}

#[test]
fn codes_without_their_own_status_are_shown_as_modified() {
    assert_eq!(parse("C "), Status::Modified);
}
