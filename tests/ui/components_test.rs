use flux::model::PathSegment;
use flux::ui::SidebarPlace;
use std::path::PathBuf;

#[test]
fn test_path_segment_initialization() {
    let segment = PathSegment {
        name: "Projects".to_string(),
        path: PathBuf::from("/home/user/Projects"),
    };

    assert_eq!(segment.name, "Projects");
    assert_eq!(segment.path, PathBuf::from("/home/user/Projects"));
}

#[test]
fn test_sidebar_place_regular_item() {
    let place = SidebarPlace {
        name: "Documents".to_string(),
        icon: "folder-documents-symbolic".to_string(),
        path: PathBuf::from("/home/user/Documents"),
        is_mount: false,
        is_section_label: false,
    };

    assert_eq!(place.name, "Documents");
    assert_eq!(place.icon, "folder-documents-symbolic");
    assert!(!place.is_mount);
    assert!(!place.is_section_label);
}

#[test]
fn test_sidebar_place_mount_item() {
    let place = SidebarPlace {
        name: "External Drive".to_string(),
        icon: "drive-harddisk-symbolic".to_string(),
        path: PathBuf::from("/run/media/user/backup"),
        is_mount: true,
        is_section_label: false,
    };

    assert!(place.is_mount);
    assert!(!place.is_section_label);
    assert_eq!(place.path, PathBuf::from("/run/media/user/backup"));
}

#[test]
fn test_sidebar_place_section_label() {
    let place = SidebarPlace {
        name: "Bookmarks".to_string(),
        icon: String::new(),
        path: PathBuf::new(),
        is_mount: false,
        is_section_label: true,
    };

    assert!(place.is_section_label);
    assert!(!place.is_mount);
    assert_eq!(place.name, "Bookmarks");
}

#[test]
fn test_file_item_preserves_foreign_owner_flag() {
    use flux::model::FileLoadContext;
    use flux::ui::FileItem;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    let dummy_icon = gtk::gio::Icon::for_string("folder").unwrap();

    let contexts = vec![
        FileLoadContext {
            display_name: "owned_folder".to_string(),
            sort_name: "owned_folder".to_string(),
            sort_ext: String::new(),
            target_path: PathBuf::from("/home/user/owned_folder"),
            size: 0,
            mtime: 0,
            is_dir: true,
            thumbnail_path: None,
            is_foreign_owner: false,
            expand_labels: false,
            custom_icon: None,
        },
        FileLoadContext {
            display_name: "root_folder".to_string(),
            sort_name: "root_folder".to_string(),
            sort_ext: String::new(),
            target_path: PathBuf::from("/root"),
            size: 0,
            mtime: 0,
            is_dir: true,
            thumbnail_path: None,
            is_foreign_owner: true,
            expand_labels: false,
            custom_icon: None,
        },
    ];

    let items: Vec<FileItem> = contexts
        .into_iter()
        .enumerate()
        .map(|(idx, ctx)| FileItem {
            name: ctx.display_name,
            icon: dummy_icon.clone(),
            thumbnail: None,
            is_dir: ctx.is_dir,
            path: ctx.target_path,
            icon_size: 48,
            size: ctx.size,
            mtime: ctx.mtime,
            is_editing: false,
            is_foreign_owner: ctx.is_foreign_owner,
            expand_labels: ctx.expand_labels,
            is_list_mode: false,
            is_custom_icon: false,
            active_path: Rc::new(RefCell::new(None)),
            grid_idx: idx as u32,
            max_width_chars: 20,
            grid_spacing: 10,
        })
        .collect();

    assert_eq!(items.len(), 2);
    assert!(
        !items[0].is_foreign_owner,
        "First item must not have is_foreign_owner set"
    );
    assert!(
        items[1].is_foreign_owner,
        "Second item must have is_foreign_owner set"
    );
}

#[test]
fn test_lock_icon_and_restricted_class_binding() {
    gtk::init().ok();

    use flux::ui::FileItem;
    use gtk::prelude::*;
    use relm4::typed_view::grid::RelmGridItem;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    let dummy_item: gtk::ListItem = glib::object::Object::new();
    let (mut root_box, mut widgets) = FileItem::setup(&dummy_item);

    let mut item = FileItem {
        name: "restricted_dir".to_string(),
        icon: gtk::gio::Icon::for_string("folder").unwrap(),
        thumbnail: None,
        is_dir: true,
        path: PathBuf::from("/root"),
        icon_size: 48,
        size: 0,
        mtime: 0,
        is_editing: false,
        is_foreign_owner: true,
        expand_labels: false,
        is_list_mode: false,
        is_custom_icon: false,
        active_path: Rc::new(RefCell::new(None)),
        grid_idx: 0,
        max_width_chars: 20,
        grid_spacing: 10,
    };

    item.bind(&mut widgets, &mut root_box);
    assert!(
        root_box.has_css_class("flux-card--restricted"),
        "Widget must have 'flux-card--restricted' CSS class when is_foreign_owner is true"
    );
    assert!(
        widgets.lock_icon.is_visible(),
        "lock_icon must be visible when is_foreign_owner is true"
    );

    item.is_foreign_owner = false;
    item.bind(&mut widgets, &mut root_box);
    assert!(
        !root_box.has_css_class("flux-card--restricted"),
        "Widget must NOT have 'flux-card--restricted' CSS class when is_foreign_owner is false"
    );
    assert!(
        !widgets.lock_icon.is_visible(),
        "lock_icon must NOT be visible when is_foreign_owner is false"
    );
}
