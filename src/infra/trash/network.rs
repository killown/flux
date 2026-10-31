use super::recursive::delete_recursive;
use gtk::gio;
use gtk::gio::prelude::FileExt;
use gtk::glib;

/// Deletes a network-resident path by walking it recursively.
pub fn delete_network_uri(uri: &str) -> Result<(), glib::Error> {
    crate::hit!("delete_network_uri");
    eprintln!("[Delete] network path → skipping trash, using worker-thread delete_recursive");
    let file = gio::File::for_uri(uri);
    let result = delete_recursive(&file);
    if let Err(e) = &result {
        eprintln!(
            "[Delete] delete_recursive failed uri={:?} gio_kind={:?} msg={:?}",
            file.uri().to_string(),
            e.kind::<gio::IOErrorEnum>(),
            e.message()
        );
    }
    result
}
