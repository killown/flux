function flux-theme --description "Switch Flux GTK themes and Papirus folder colors"
    if test (count $argv) -ne 1
        echo (set_color red)"Usage: flux-theme [blue|frost|orange|cyan|obsidian|monolith|crimson|smoke|dark]"(set_color normal)
        return 1
    end

    set -l color $argv[1]
    set -l gtk_theme ""
    set -l papirus_color ""

    switch $color
        case blue
            set gtk_theme Flux-Blue
            set papirus_color blue
        case frost
            set gtk_theme Flux-Frost
            set papirus_color palebrown
        case orange
            set gtk_theme Flux-Forge
            set papirus_color orange
        case cyan
            set gtk_theme Flux-Cyan
            set papirus_color cyan
        case obsidian
            set gtk_theme Flux-Obsidian
            set papirus_color black
        case monolith
            set gtk_theme Flux-Monolith
            set papirus_color white
        case crimson
            set gtk_theme Flux-Crimson
            set papirus_color carmine
        case smoke
            set gtk_theme Flux-Smoke
            set papirus_color grey
        case dark
            set gtk_theme os-dark
            set papirus_color nordic
        case '*'
            echo (set_color red)"❌ Unknown theme: $color"(set_color normal)
            echo "Available: blue, frost, orange, cyan, obsidian, monolith, crimson, smoke, dark"
            return 1
    end

    echo (set_color cyan)"🎨 Switching Flux theme to: "(set_color --bold white)"$color"(set_color normal)

    flux-fm --theme "$gtk_theme"
    echo (set_color green)"✅ GTK Theme set to $gtk_theme"(set_color normal)

    if type -q papirus-folders
        papirus-folders -C $papirus_color --theme Papirus
        echo (set_color green)"✅ Folders set to $papirus_color"(set_color normal)
    else
        echo (set_color yellow)"⚠️ papirus-folders not found. Skipping folder color."(set_color normal)
    end

    gtk-update-icon-cache -f -t ~/.icons/Papirus &>/dev/null
    echo (set_color green)"✅ Icon cache updated"(set_color normal)

    echo (set_color green --bold)"🚀 Flux theme applied successfully!"(set_color normal)
end
