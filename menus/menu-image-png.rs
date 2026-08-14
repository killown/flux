# --- PNG Optimization ---
"󰕧      Optimize PNG (pngquant)"     => "image/png", "pngquant --ext .png --force 256 %p", "PNG compressed!"
"󰕧      Strip Metadata (exiftool)"   => "image/png", "exiftool -all= %p", "Exif metadata removed"

# --- Conversions ---
"󰸉      Convert > To WebP"            => "image/png", "magick %p -quality 80 %p.webp", "Converted to WebP"
"󰸉      Convert > To AVIF"            => "image/png", "avifenc --jobs all -q 65 %p %p.avif", "Converted to AVIF"

# --- Editors ---
"󰸉      Open in GIMP"                => "image/png", "gimp %p", "", "no_command_dialog"
