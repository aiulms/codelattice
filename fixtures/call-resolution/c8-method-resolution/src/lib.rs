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

    pub fn value(&self) -> i32 {
        self.count
    }
}

pub fn use_counter() {
    let mut c = Counter::new();
    c.increment();
    let _v = c.value();
}
