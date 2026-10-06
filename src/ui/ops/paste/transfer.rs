use adw::gio::prelude::*;
use gtk::gio;
use std::path::{Path, PathBuf};

pub(super) fn resolve_gio_files(files: Vec<gio::File>) -> Vec<(PathBuf, String, bool)> {
    files
        .into_iter()
        .filter_map(|file| {
            let src_path = file.path().or_else(|| {
                let uri = file.uri().to_string();
                let clean_uri = uri.trim_end_matches('/');
                gio::File::for_uri(clean_uri).path()
            })?;

            let orig_name = src_path.file_name()?.to_string_lossy().to_string();
            let clean_name = clean_tmp_basename(&orig_name);
            let is_dir = src_path.is_dir();
            Some((src_path, clean_name, is_dir))
        })
        .collect()
}

pub fn perform_file_op(
    src: &Path,
    dest: &Path,
    is_cut: bool,
    cancellable: &gio::Cancellable,
) -> Result<(), String> {
    perform_file_op_with_progress(src, dest, is_cut, cancellable, None)
}

pub fn perform_file_op_with_progress(
    src: &Path,
    dest: &Path,
    is_cut: bool,
    cancellable: &gio::Cancellable,
    mut progress_cb: Option<&mut dyn FnMut(i64, i64)>,
) -> Result<(), String> {
    if !src.exists() {
        return Err(format!("Source '{}' does not exist", src.display()));
    }
    if src == dest {
        return Ok(());
    }

    if src.is_dir() && is_strictly_inside(src, dest) {
        return Err("Cannot copy or move a folder into itself".to_string());
    }

    let src_file = gio::File::for_path(src);
    let dst_file = gio::File::for_path(dest);

    if is_cut {
        match src_file.move_(
            &dst_file,
            gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::ALL_METADATA,
            Some(cancellable),
            match progress_cb.as_mut() {
                Some(f) => Some(&mut **f),
                None => None,
            },
        ) {
            Ok(()) => return Ok(()),

            Err(e) if cancellable.is_cancelled() || e.matches(gio::IOErrorEnum::Cancelled) => {
                if !src.is_dir() {
                    let _ = std::fs::remove_file(dest);
                }
                return Err(e.to_string());
            }
            Err(_) => {}
        }
    }

    let mut created: Vec<PathBuf> = Vec::new();

    let copy_res = if src.is_dir() {
        copy_dir_recursive(
            src,
            dest,
            cancellable,
            match progress_cb.as_mut() {
                Some(f) => Some(&mut **f),
                None => None,
            },
            &mut created,
        )
        .map_err(|e| e.to_string())
    } else {
        if dest.symlink_metadata().is_err() {
            created.push(dest.to_path_buf());
        }
        src_file
            .copy(
                &dst_file,
                gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                Some(cancellable),
                match progress_cb.as_mut() {
                    Some(f) => Some(&mut **f),
                    None => None,
                },
            )
            .map_err(|e| e.to_string())
    };

    if copy_res.is_err() {
        if !src.is_dir() {
            let _ = std::fs::remove_file(dest);
        } else {
            rollback_created(&created);
        }
        return copy_res;
    }

    if is_cut {
        let _ = if src.is_dir() {
            std::fs::remove_dir_all(src)
        } else {
            std::fs::remove_file(src)
        };
    }

    Ok(())
}

/// Canonicalises `p`, falling back to parent + filename when it does not exist.
pub(super) fn resolve_for_compare(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    match (p.parent(), p.file_name()) {
        (Some(parent), Some(name)) => parent
            .canonicalize()
            .map(|c| c.join(name))
            .unwrap_or_else(|_| p.to_path_buf()),
        _ => p.to_path_buf(),
    }
}

/// True when `dest` is beneath `src`. `dest == src` returns false.
pub(super) fn is_strictly_inside(src: &Path, dest: &Path) -> bool {
    let s = resolve_for_compare(src);
    let d = resolve_for_compare(dest);
    d != s && d.starts_with(&s)
}

/// Removes `created` in reverse. Dirs use `remove_dir`, so a non-empty dir
/// added by someone else is left alone.
pub(super) fn rollback_created(created: &[PathBuf]) {
    for p in created.iter().rev() {
        match p.symlink_metadata() {
            Ok(m) if m.is_dir() => {
                let _ = std::fs::remove_dir(p);
            }
            Ok(_) => {
                let _ = std::fs::remove_file(p);
            }
            Err(_) => {}
        }
    }
}

pub(super) fn copy_dir_recursive(
    src: &Path,
    dest: &Path,
    cancellable: &gio::Cancellable,
    mut progress_cb: Option<&mut dyn FnMut(i64, i64)>,
    created: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if src == dest {
        return Ok(());
    }
    if dest.symlink_metadata().is_err() {
        std::fs::create_dir_all(dest)?;
        created.push(dest.to_path_buf());
    } else if !dest.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} exists and is not a folder", dest.display()),
        ));
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;

        if cancellable.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }

        let child_src = entry.path();
        let child_dest = dest.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(
                &child_src,
                &child_dest,
                cancellable,
                match progress_cb.as_mut() {
                    Some(f) => Some(&mut **f),
                    None => None,
                },
                created,
            )?;
        } else {
            if child_dest.symlink_metadata().is_err() {
                created.push(child_dest.clone());
            }
            gio::File::for_path(&child_src)
                .copy(
                    &gio::File::for_path(&child_dest),
                    gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    Some(cancellable),
                    match progress_cb.as_mut() {
                        Some(f) => Some(&mut **f),
                        None => None,
                    },
                )
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }

    Ok(())
}

pub(super) fn scan_total_bytes(path: &Path) -> u64 {
    fn walk_dir(dir: &Path) -> u64 {
        let mut total = 0u64;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let Ok(meta) = entry.path().symlink_metadata() else {
                    continue;
                };
                if meta.file_type().is_dir() {
                    total += walk_dir(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
        total
    }

    match std::fs::metadata(path).or_else(|_| std::fs::symlink_metadata(path)) {
        Ok(m) if m.is_dir() => walk_dir(path),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

pub(super) fn clean_tmp_basename(name: &str) -> String {
    if name.starts_with(".tmp") {
        name.split_once('.')
            .and_then(|(_, rest)| rest.split_once('.'))
            .map(|(_, real)| real.to_string())
            .unwrap_or_else(|| name.to_string())
    } else {
        name.to_string()
    }
}

pub(super) fn is_cancelled_error(cancellable: &gio::Cancellable, msg: &str) -> bool {
    cancellable.is_cancelled()
        || msg.contains("g-io-error-quark: 19")
        || msg.contains("g-io-error-quark:19")
}
