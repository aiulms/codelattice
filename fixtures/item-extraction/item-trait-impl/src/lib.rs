pub trait Display {
    fn fmt(&self) -> String;
}

pub struct Point {
    x: f64,
    y: f64,
}

impl Display for Point {
    fn fmt(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}

impl Point {
    pub fn origin() -> Self {
        Point { x: 0.0, y: 0.0 }
    }

    pub fn distance(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

