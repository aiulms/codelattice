macro_rules! create_fn {
    ($name:ident) => { fn $name() {} };
}

fn normal_fn() {}

// macro invocation
create_fn!(generated_fn);

println!("test");
