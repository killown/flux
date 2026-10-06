use glib::subclass::types::ObjectSubclassIsExt;
use gtk::glib;
use std::cell::RefCell;

mod imp {
    use super::*;
    use glib::subclass::prelude::*;

    #[derive(Default)]
    pub struct NerdIconObject {
        pub glyph: RefCell<String>,
        pub name: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NerdIconObject {
        const NAME: &'static str = "FluxNerdIconObject";
        type Type = super::NerdIconObject;
    }

    impl ObjectImpl for NerdIconObject {}
}

glib::wrapper! {
    pub struct NerdIconObject(ObjectSubclass<imp::NerdIconObject>);
}

impl NerdIconObject {
    pub fn new(glyph: &str, name: &str) -> Self {
        let obj: Self = glib::Object::builder().build();
        *obj.imp().glyph.borrow_mut() = glyph.to_string();
        *obj.imp().name.borrow_mut() = name.to_string();
        obj
    }

    pub fn glyph(&self) -> String {
        self.imp().glyph.borrow().clone()
    }

    pub fn name(&self) -> String {
        self.imp().name.borrow().clone()
    }
}
