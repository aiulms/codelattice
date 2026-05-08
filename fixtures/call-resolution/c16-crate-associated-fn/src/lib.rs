// compile-valid: 验证 crate:: 路径的 AssociatedFunction vs QualifiedPath 分类
// crate::module::Type::method() 应分类为 AssociatedFunction
// crate::module::function() 应分类为 QualifiedPath

mod inner;

/// 关联函数调用 crate::inner::MyType::build()
/// segments: [crate, inner, MyType, build] = 4段, second_last=MyType(大写) → AssociatedFunction
pub fn test_associated_fn() -> crate::inner::MyType {
    crate::inner::MyType::build("test")
}

/// 限定路径自由函数调用 crate::inner::helper()
/// segments: [crate, inner, helper] = 3段 → QualifiedPath
pub fn test_qualified_path() -> u32 {
    crate::inner::helper(10)
}
