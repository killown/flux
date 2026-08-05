#!/usr/bin/env python3
"""Convert images and office documents into PDF format using img2pdf, ImageMagick, or LibreOffice."""

import os
import subprocess
import sys


def main() -> None:
    """Process command-line file arguments and convert supported formats to PDF."""
    if len(sys.argv) < 2:
        print("No files selected.")
        sys.exit(1)

    files = sys.argv[1:]

    ext = os.path.splitext(files[0])[1].lower()

    images_ext = [".jpg", ".jpeg", ".png", ".webp", ".bmp"]
    docs_ext = [
        ".doc",
        ".docx",
        ".odt",
        ".rtf",
        ".txt",
        ".ppt",
        ".pptx",
        ".xls",
        ".xlsx",
    ]

    if ext in images_ext:
        output_pdf = os.path.join(os.path.dirname(files[0]), "combined_document.pdf")
        try:
            import img2pdf

            with open(output_pdf, "wb") as f:
                f.write(img2pdf.convert(files))
            print(f"PDF successfully created: {output_pdf}")
        except ImportError:
            subprocess.run(["magick"] + files + [output_pdf])

    elif ext in docs_ext:
        for file in files:
            # Get absolute directory path to prevent issues with empty string ""
            out_dir = os.path.abspath(os.path.dirname(file))

            subprocess.run(
                [
                    "libreoffice",
                    "--headless",
                    "--convert-to",
                    "pdf:writer_pdf_Export",  # Force the exact Writer PDF filter without code interpretation
                    file,
                    "--outdir",
                    out_dir,
                ]
            )
        print("Documents converted to PDF!")
    else:
        print(f"Format not supported for direct conversion: {ext}")


if __name__ == "__main__":
    main()
