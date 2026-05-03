pub struct Counter {
    count: i32,
}

impl Counter {
    pub fn new() -> Self {
        Counter { count: 0 }
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }
}

pub fn use_counter() {
    let mut c = Counter::new();
    c.increment();
}
