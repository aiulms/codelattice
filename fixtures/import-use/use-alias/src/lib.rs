mod foo;

use crate::foo::Bar as RenamedBar;

pub fn hello() -> RenamedBar {
    RenamedBar::new()
}
