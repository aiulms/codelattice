// compile-valid: 验证 inline module 的 self::/super::/crate:: 路径 CALLS 图产出
// 覆盖 inline module 内的不同路径类型

/// 顶层函数，供 inline module 通过 super:: 和 crate:: 路径调用
pub fn root_fn() -> u32 {
    42
}

/// inline 模块：验证 self:: 和 super:: 路径
pub mod inner {
    /// inner 模块的函数
    pub fn inner_fn() -> u32 {
        10
    }

    /// 通过 self:: 调用同模块函数
    pub fn call_self() -> u32 {
        self::inner_fn()
    }

    /// 通过 super:: 调用顶层函数
    pub fn call_super() -> u32 {
        super::root_fn()
    }

    /// 嵌套 inline 模块：验证 super:: 到父模块 + crate:: 到 root
    pub mod nested {
        /// 通过 super:: 调用父模块（inner）的函数
        pub fn call_super_to_parent() -> u32 {
            super::inner_fn()
        }

        /// 通过 crate:: 调用顶层函数
        pub fn call_crate() -> u32 {
            crate::root_fn()
        }
    }
}
