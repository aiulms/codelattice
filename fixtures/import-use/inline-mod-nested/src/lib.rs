mod a {
    mod b {
        use self::X;
        use super::Y;
    }
    struct Y;
}
struct X;
