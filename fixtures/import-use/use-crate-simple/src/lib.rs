mod foo;

use crate::foo::Bar;

pub fn hello() -> Bar {
    Bar::new()
}
