#[derive(Debug, Clone, Copy)]
pub struct PanelSpec {
    pub initial_width: i32,
    pub min_width: i32,
    pub max_width: i32,
    pub default_width: i32,
}

impl PanelSpec {
    pub const fn new(min_width: i32, max_width: i32, default_width: i32) -> Self {
        Self {
            initial_width: 0,
            min_width,
            max_width,
            default_width,
        }
    }

    pub const fn with_initial(mut self, w: i32) -> Self {
        self.initial_width = w;
        self
    }

    pub fn effective(&self) -> i32 {
        if self.initial_width <= 0 {
            self.default_width
        } else {
            self.initial_width.clamp(self.min_width, self.max_width)
        }
    }

    pub fn clamp(&self, w: i32) -> i32 {
        w.clamp(self.min_width, self.max_width)
    }
}

impl PanelSpec {
    pub const fn tag() -> Self {
        Self::new(250, 800, 350)
    }
    pub const fn diff() -> Self {
        Self::new(280, 900, 420)
    }
    pub const fn search() -> Self {
        Self::new(250, 800, 350)
    }
}
