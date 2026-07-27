// 复杂度热点诊断 fixture
// 包含：1 个超长函数（>50 行，触发 length 维度）、1 个短函数（对照）、
// 多个 helper 制造 fan-in/fan-out。
// 注意：函数长度是 raw span（含注释/空行），这是 v1 的明示语义。

/// 超长函数：>50 行，触发 long-function driver + async modifier 加权。
pub async fn oversized_async_function() -> i32 {
    // line 8
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    let f = 6;
    let g = 7;
    let h = 8;
    let i = 9;
    let j = 10;
    let k = 11;
    let l = 12;
    let m = 13;
    let n = 14;
    let o = 15;
    let p = 16;
    let q = 17;
    let r = 18;
    let s = 19;
    let t = 20;
    let u = 21;
    let v = 22;
    let w = 23;
    let x = 24;
    let y = 25;
    let z = 26;
    let aa = 27;
    let bb = 28;
    let cc = 29;
    let dd = 30;
    let ee = 31;
    let ff = 32;
    let gg = 33;
    let hh = 34;
    let ii = 35;
    let jj = 36;
    let kk = 37;
    let ll = 38;
    let mm = 39;
    let nn = 40;
    let oo = 41;
    let pp = 42;
    let qq = 43;
    let rr = 44;
    let ss = 45;
    let tt = 46;
    let uu = 47;
    let vv = 48;
    let ww = 49;
    let xx = 50;
    let result = a + b + c + d + e;
    result
}

/// 超长 unsafe 函数：>50 行，触发 long-function driver + unsafe modifier 加权。
pub unsafe fn oversized_unsafe_function() -> i32 {
    // line 61
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    let f = 6;
    let g = 7;
    let h = 8;
    let i = 9;
    let j = 10;
    let k = 11;
    let l = 12;
    let m = 13;
    let n = 14;
    let o = 15;
    let p = 16;
    let q = 17;
    let r = 18;
    let s = 19;
    let t = 20;
    let u = 21;
    let v = 22;
    let w = 23;
    let x = 24;
    let y = 25;
    let z = 26;
    let aa = 27;
    let bb = 28;
    let cc = 29;
    let dd = 30;
    let ee = 31;
    let ff = 32;
    let gg = 33;
    let hh = 34;
    let ii = 35;
    let jj = 36;
    let kk = 37;
    let ll = 38;
    let mm = 39;
    let nn = 40;
    let oo = 41;
    let pp = 42;
    let qq = 43;
    let rr = 44;
    let ss = 45;
    let tt = 46;
    let uu = 47;
    let vv = 48;
    let ww = 49;
    let xx = 50;
    let result = a + b + c + d + e;
    result
}

/// 短函数：3 行，不应被报告（低于 medium 阈值）。
pub fn tiny_helper() -> i32 {
    42
}

/// 被 oversized_function 调用的 helper（制造 fan-in）。
pub fn called_helper_one() -> i32 {
    1
}

pub fn called_helper_two() -> i32 {
    2
}
