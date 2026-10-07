use gtk::prelude::*;

pub fn panel_header(
    icon: Option<&str>,
    title: &str,
    extra: &[gtk::Widget],
    on_close: impl Fn() + 'static,
) -> gtk::Box {
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(6)
        .build();

    if let Some(icon_name) = icon {
        header_box.append(&gtk::Image::from_icon_name(icon_name));
    }

    let title_label = gtk::Label::builder()
        .label(title)
        .css_classes(["heading"])
        .hexpand(true)
        .xalign(0.0)
        .build();
    header_box.append(&title_label);

    for w in extra {
        header_box.append(w);
    }

    let close_btn = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(crate::i18n::tr("Close"))
        .build();
    close_btn.connect_clicked(move |_| on_close());
    header_box.append(&close_btn);

    header_box
}
