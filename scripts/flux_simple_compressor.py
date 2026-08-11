#!/usr/bin/env python3

import os
import sys
import argparse
import zipfile
import tarfile
import datetime
from pathlib import Path


def get_archive_writer(format, path):
    if format == "zip":
        return (lambda p: zipfile.ZipFile(p, "w", zipfile.ZIP_DEFLATED), ".zip")
    elif format == "tar":
        return (lambda p: tarfile.open(p, "w"), ".tar")
    elif format == "gztar":
        return (lambda p: tarfile.open(p, "w:gz"), ".tar.gz")
    elif format == "bztar":
        return (lambda p: tarfile.open(p, "w:bz2"), ".tar.bz2")
    elif format == "xztar":
        return (lambda p: tarfile.open(p, "w:xz"), ".tar.xz")
    else:
        raise ValueError(f"Unsupported format: {format}")


def add_path_to_archive(writer, path, arcname=None):
    path = Path(path).resolve()
    if arcname is None:
        arcname = path.name

    if path.is_dir():
        for entry in path.rglob("*"):
            if entry.is_file():
                rel = entry.relative_to(path)
                writer.write(entry, os.path.join(arcname, rel))
    else:
        writer.write(path, arcname)


def main():
    parser = argparse.ArgumentParser(
        description="Compress files/directories into an archive."
    )
    parser.add_argument("paths", nargs="+", help="Files/directories to compress")
    parser.add_argument(
        "-f",
        "--format",
        choices=["zip", "tar", "gztar", "bztar", "xztar"],
        default="zip",
        help="Archive format (default: zip)",
    )
    parser.add_argument(
        "-o", "--output", help="Output directory (default: parent of first path)"
    )
    parser.add_argument(
        "-n", "--name", help="Base name for archive (without extension)"
    )
    args = parser.parse_args()

    if not args.paths:
        print("Error: no paths given.", file=sys.stderr)
        sys.exit(1)

    abs_paths = []
    for p in args.paths:
        expanded_p = os.path.expanduser(p)
        abs_p = Path(expanded_p).resolve()
        if not abs_p.exists():
            print(f"Warning: {abs_p} does not exist, skipping.", file=sys.stderr)
            continue
        abs_paths.append(abs_p)

    if not abs_paths:
        print("Error: no valid files/folders to compress.", file=sys.stderr)
        sys.exit(1)

    if args.output:
        expanded_out = os.path.expanduser(args.output)
        out_dir = Path(expanded_out).resolve()
    else:
        out_dir = abs_paths[0].parent

    out_dir.mkdir(parents=True, exist_ok=True)

    if args.name:
        base_name = args.name
    else:
        base_name = abs_paths[0].stem
        ts = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
        base_name = f"{base_name}_{ts}"

    writer_fn, ext = get_archive_writer(args.format, None)
    archive_path = out_dir / f"{base_name}{ext}"

    with writer_fn(archive_path) as archive:
        for path in abs_paths:
            add_path_to_archive(archive, path, arcname=path.name)

    print(f"Created: {archive_path}")
    sys.exit(0)


if __name__ == "__main__":
    main()
