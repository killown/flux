# --- Wallpaper & Display ---
"󰸉      Set as Wallpaper (swww)" => "image/all", "swww img %p", "Wallpaper updated!"
"󰸉      Set as Wallpaper (wbg)"  => "image/all", "cp %p ~/Images/fav.jpg && wbg -s ~/Images/fav.jpg", "Wallpaper set (wbg)"

# --- Quick Conversions ---
"󰸉      Convert > To WebP" => "image/all", "magick %p -quality 80 %p.webp", "Converted to WebP"
"󰸉      Convert > To AVIF" => "image/all", "avifenc --jobs all -q 65 %p %p.avif", "Converted to AVIF"
"󰸉      Convert > To JPG"  => "image/all", "magick %p -quality 85 -strip %p.jpg", "Converted to JPG"
"󰏦      Convert > To PDF"  => "image/all", "magick %p %p.pdf", "Converted to PDF"

# --- Editors ---
"󰸉      Open > In GIMP"     => "image/all", "gimp %p", "", "no_command_dialog"
"󰸉      Open > In Inkscape" => "image/all", "inkscape %p", "", "no_command_dialog"
