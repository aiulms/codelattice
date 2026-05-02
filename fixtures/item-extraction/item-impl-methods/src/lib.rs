pub struct User {
    name: String,
}

impl User {
    pub fn new(name: String) -> Self {
        User { name }
    }

    pub fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }

    fn internal(&mut self) {}
}

