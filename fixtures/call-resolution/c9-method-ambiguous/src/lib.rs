pub struct A;
impl A {
    pub fn reset(&self) {}
}

pub struct B;
impl B {
    pub fn reset(&self) {}
}

pub fn use_both() {
    let a = A;
    a.reset();
}
