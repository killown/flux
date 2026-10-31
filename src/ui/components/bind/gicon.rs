//! `gio::Icon` -> `Paintable` resolution for a bound cell.
//!
//! # Contract
//!
//! Never decode on a cache miss, paint a theme fallback right away, and
//! dispatch the decode to a worker. See the block comment in
//! [`paintable_for_gicon`] before changing the custom-icon path.

use crate::ui::components::FileItem;
use adw::gdk;
use adw::prelude::*;

/// Resolves `icon` to a paintable at this item's icon size.
///
/// Shared by the normal icon paint and by the virtual-dir folder/original
/// pair, so all of them obey the same rules: never decode on a cache miss,
/// paint a theme fallback right away, and dispatch the decode to a worker.
pub(super) fn paintable_for_gicon(
    item: &FileItem,
    root: &gtk::Overlay,
    icon: &gtk::gio::Icon,
) -> gdk::Paintable {
    // GFileIcon paintables come back at the file's natural size, which would
    // stretch the grid cell, so custom file icons are loaded as textures and
    // scaled by hand.
    let custom_path = icon
        .downcast_ref::<gtk::gio::FileIcon>()
        .and_then(|fi| fi.file().path());

    // Custom icon painting. The decode has to stay off the main thread: decoding
    // at full source resolution inside `bind` once cost tens of milliseconds per
    // call, and a single 4K PNG was enough to drop frames.
    //
    //   * `peek_cached_file_icon_texture` returns `None` on a miss and never
    //     decodes.
    //   * The cache key includes `mtime`, so editing an icon file takes effect
    //     without a restart.
    //   * Raster decode goes through `decode_to_pixbuf` (progressive downscale).
    //     Don't inline `from_file_at_scale`, it decodes at source resolution.
    //   * Decodes run through the thumbnail semaphore, so a folder full of large
    //     custom icons can't starve thumbnails and file ops on the shared pool.
    //   * The handler forces a rebind (remove + insert). `queue_draw()` re-runs
    //     draw, not bind, so the fallback icon would stay on screen.
    //   * Decodes are deduplicated per path (`begin_file_icon_decode`), since every
    //     file with the same extension shares one icon.
    //
    // To verify: `FLUX_HWGA_OUT=1 flux-fm <dir-with-custom-icons>`, then check that
    // `bind::bind_icon_paint` stays sub-millisecond with `Main>16ms` at `0/N`.
    let Some(path) = custom_path else {
        return lookup_theme_icon(item, root, icon);
    };

    // Fast path: texture already decoded and cached on the main thread.
    // No I/O, no decode, just a HashMap lookup and a refcount bump.
    if let Some(texture) = crate::utils::icon::peek_cached_file_icon_texture(&path) {
        return texture.upcast::<gdk::Paintable>();
    }

    // Slow path: not cached. Paint the theme fallback immediately so
    // the cell is never blank, then dispatch the decode to a worker.
    //
    // WARNING: the decode is throttled through the thumbnail
    // semaphore so a folder full of custom icons (often >1MB
    // photos) cannot starve thumbnails/file-ops on the shared
    // blocking pool. Do not remove the `acquire_blocking`, it is
    // what keeps `spawn_blocking` from being flooded by N parallel
    // decodes.
    //
    // WARNING: the decode goes through `decode_to_pixbuf`, which
    // uses `image::thumbnail` for raster formats (progressive
    // downscale during decode, ~3-5x faster than
    // `from_file_at_scale` on large sources) and GdkPixbuf for
    // SVG. Do not inline a `from_file_at_scale` call here, it
    // decodes at source resolution and is what caused the
    // original ~40ms stall.
    let fallback = lookup_theme_icon(item, root, icon);

    let decode_guard = crate::utils::icon::begin_file_icon_decode(&path);
    if let (Some(sender), Some(decode_guard)) =
        (crate::app::state::SENDER.get().cloned(), decode_guard)
    {
        let path_clone = path.clone();
        let thumbnail_manager = item.thumbnail_manager.clone();
        let max_threads = item.thumbnail_threads;
        relm4::spawn_blocking(move || {
            crate::hit!("decode_custom_file_icon");
            let Some(_permit) = thumbnail_manager.acquire_blocking(max_threads) else {
                return;
            };
            if let Some(pixbuf) = crate::utils::media::decode_to_pixbuf(&path_clone, 256) {
                let _ = sender.send(crate::domain::model::AppMsg::Thumbnails(
                    crate::domain::model::ThumbnailsMsg::FileIconTextureReady {
                        path: path_clone,
                        image: crate::utils::media::image_from_texture(&gdk::Texture::for_pixbuf(
                            &pixbuf,
                        )),
                    },
                ));
                decode_guard.disarm();
            }
        });
    }

    fallback
}

/// Plain icon-theme lookup at the item's size and the root's scale factor.
fn lookup_theme_icon(
    item: &FileItem,
    root: &gtk::Overlay,
    icon: &gtk::gio::Icon,
) -> gdk::Paintable {
    let display = root.display();
    let icon_theme = gtk::IconTheme::for_display(&display);
    let scale = root.scale_factor().max(1);
    icon_theme
        .lookup_by_gicon(
            icon,
            item.icon_size,
            scale,
            gtk::TextDirection::None,
            gtk::IconLookupFlags::empty(),
        )
        .upcast::<gdk::Paintable>()
}
