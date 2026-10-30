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
    // Custom file-path icons must be loaded as textures and scaled ourselves,
    // because GtkIconTheme::lookup_by_gicon returns a paintable at the file's
    // natural size for GFileIcon, which the Image widget then paints verbatim
    // (stretching the grid cell's width).
    // NOTE: This is necessary, otherwise the grid layout will be inconsistent and the icon will
    // not match the requested size.
    let custom_path = icon
        .downcast_ref::<gtk::gio::FileIcon>()
        .and_then(|fi| fi.file().path());

    // ── Custom file-icon painting: keep the decode OFF the main thread ────────
    //
    // NOTE: History: this used to be the single worst main-thread stall in folder
    // loading. Back then we called `gdk::Texture::from_filename(&path)` right
    // here, inside `bind`, once per grid cell. That decodes the icon at its
    // full source resolution on the GTK thread. `hwga` flagged it at ~42ms
    // total, with a 40.7ms single-call max and 1/22 calls blowing the 16ms
    // frame budget, all inside `append_context_batch` while populating a
    // folder. A single 4K PNG custom icon was enough to drop frames.
    //
    // What the shape of this block is protecting:
    //
    //   * `peek_cached_file_icon_texture` must return `None` on a miss. It
    //     must NOT decode. Decoding on a miss puts the 40ms stall back into
    //     every bind.
    //   * The cache key must include `mtime`. Without it, editing a custom
    //     icon file has no visible effect until the app restarts.
    //   * Raster decode goes through `decode_to_pixbuf` (image crate,
    //     progressive downscale). Do not inline `from_file_at_scale` - it
    //     decodes at source resolution (~20ms for a 4K PNG) and is what the
    //     256px bound was papering over.
    //   * The decode must be throttled through the thumbnail semaphore. A
    //     folder with hundreds of >1MB custom icons will otherwise starve
    //     thumbnails and file-ops on the shared blocking pool.
    //   * The handler must force a rebind (remove + insert). `queue_draw()`
    //     re-runs draw, not bind, so the fallback icon stays on screen.
    //   * Decodes are deduplicated per path (`begin_file_icon_decode`). Every
    //     file with the same extension shares one icon, so without it a folder
    //     of N such files decodes the image N times and rebinds O(N^2) cells.
    //
    // If you touch this block, verify with:
    //   FLUX_HWGA_OUT=1 flux-fm <dir-with-custom-icons>
    // and check `bind::bind_icon_paint` - total sub-millisecond, `Main>16ms`
    // reads `0/N`.
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
    if let (Some(sender), Some(decode_guard)) = (crate::model::SENDER.get().cloned(), decode_guard)
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
                let _ = sender.send(crate::model::AppMsg::FileIconTextureReady {
                    path: path_clone,
                    texture: gdk::Texture::for_pixbuf(&pixbuf),
                });
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
