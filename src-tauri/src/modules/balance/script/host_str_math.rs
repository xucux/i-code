//! # 字符串 / 数学 Host Functions
//!
//! `str.*` / `math.*` 以及扁平别名，供额度脚本使用。

use rhai::Engine;

/// 注册字符串与数学函数
///
/// - 扁平名：`str_contains` / `math_abs` 等
/// - 静态模块：`str::contains` / `math::abs` 等
///
/// 注意：模块调用请用 `str::trim(...)` / `math::min(...)`，不要写 `str.trim`。
pub fn register(engine: &mut Engine) {
    // ===== 扁平函数（便于文档与补全）=====
    engine.register_fn("str_contains", |text: &str, sub: &str| -> bool {
        text.contains(sub)
    });
    engine.register_fn("str_replace", |text: &str, from: &str, to: &str| -> String {
        text.replace(from, to)
    });
    engine.register_fn("str_starts_with", |text: &str, prefix: &str| -> bool {
        text.starts_with(prefix)
    });
    engine.register_fn("str_ends_with", |text: &str, suffix: &str| -> bool {
        text.ends_with(suffix)
    });
    engine.register_fn("str_trim", |text: &str| -> String { text.trim().to_string() });
    engine.register_fn("str_to_lower", |text: &str| -> String {
        text.to_lowercase()
    });
    engine.register_fn("str_to_upper", |text: &str| -> String {
        text.to_uppercase()
    });
    engine.register_fn("str_len", |text: &str| -> i64 { text.chars().count() as i64 });
    engine.register_fn(
        "str_sub_string",
        |text: &str, start: i64, end: i64| -> String {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len() as i64;
            let s = start.clamp(0, len) as usize;
            let e = end.clamp(0, len) as usize;
            if s >= e {
                String::new()
            } else {
                chars[s..e].iter().collect()
            }
        },
    );
    // 字符串 → 浮点：解析失败自动抛错（额度脚本中常见，如 "80138.77"）
    engine.register_fn(
        "str_to_float",
        |text: &str| -> Result<f64, Box<rhai::EvalAltResult>> {
            text.trim().parse::<f64>().map_err(|e| {
                format!("str_to_float: 无法解析 '{text}' 为浮点数: {e}")
                    .into()
            })
        },
    );
    // 字符串 → 整数：解析失败自动抛错
    engine.register_fn(
        "str_to_int",
        |text: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
            text.trim().parse::<i64>().map_err(|e| {
                format!("str_to_int: 无法解析 '{text}' 为整数: {e}").into()
            })
        },
    );

    engine.register_fn("math_abs", |x: f64| -> f64 { x.abs() });
    engine.register_fn("math_min", |a: f64, b: f64| -> f64 { a.min(b) });
    engine.register_fn("math_max", |a: f64, b: f64| -> f64 { a.max(b) });
    engine.register_fn("math_floor", |x: f64| -> f64 { x.floor() });
    engine.register_fn("math_ceil", |x: f64| -> f64 { x.ceil() });
    engine.register_fn("math_round", |x: f64| -> f64 { x.round() });
    engine.register_fn("math_sqrt", |x: f64| -> f64 { x.sqrt() });
    engine.register_fn("math_pow", |base: f64, exp: f64| -> f64 { base.powf(exp) });

    // i64 重载
    engine.register_fn("math_abs", |x: i64| -> i64 { x.abs() });
    engine.register_fn("math_min", |a: i64, b: i64| -> i64 { a.min(b) });
    engine.register_fn("math_max", |a: i64, b: i64| -> i64 { a.max(b) });

    // ===== 模块风格：str.contains / math.abs =====
    let mut str_mod = rhai::Module::new();
    str_mod.set_native_fn("contains", |text: &str, sub: &str| Ok(text.contains(sub)));
    str_mod.set_native_fn("replace", |text: &str, from: &str, to: &str| {
        Ok::<_, Box<rhai::EvalAltResult>>(text.replace(from, to))
    });
    str_mod.set_native_fn("starts_with", |text: &str, prefix: &str| {
        Ok(text.starts_with(prefix))
    });
    str_mod.set_native_fn("ends_with", |text: &str, suffix: &str| {
        Ok(text.ends_with(suffix))
    });
    str_mod.set_native_fn("trim", |text: &str| {
        Ok::<_, Box<rhai::EvalAltResult>>(text.trim().to_string())
    });
    str_mod.set_native_fn("to_lower", |text: &str| {
        Ok::<_, Box<rhai::EvalAltResult>>(text.to_lowercase())
    });
    str_mod.set_native_fn("to_upper", |text: &str| {
        Ok::<_, Box<rhai::EvalAltResult>>(text.to_uppercase())
    });
    str_mod.set_native_fn("len", |text: &str| Ok(text.chars().count() as i64));
    str_mod.set_native_fn("sub_string", |text: &str, start: i64, end: i64| {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len() as i64;
        let s = start.clamp(0, len) as usize;
        let e = end.clamp(0, len) as usize;
        if s >= e {
            Ok::<_, Box<rhai::EvalAltResult>>(String::new())
        } else {
            Ok(chars[s..e].iter().collect())
        }
    });
    str_mod.set_native_fn("to_float", |text: &str| -> Result<f64, Box<rhai::EvalAltResult>> {
        text.trim().parse::<f64>().map_err(|e| {
            format!("str::to_float: 无法解析 '{text}' 为浮点数: {e}").into()
        })
    });
    str_mod.set_native_fn("to_int", |text: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        text.trim().parse::<i64>().map_err(|e| {
            format!("str::to_int: 无法解析 '{text}' 为整数: {e}").into()
        })
    });
    engine.register_static_module("str", str_mod.into());

    let mut math_mod = rhai::Module::new();
    math_mod.set_native_fn("abs", |x: f64| Ok(x.abs()));
    math_mod.set_native_fn("min", |a: f64, b: f64| Ok(a.min(b)));
    math_mod.set_native_fn("max", |a: f64, b: f64| Ok(a.max(b)));
    math_mod.set_native_fn("floor", |x: f64| Ok(x.floor()));
    math_mod.set_native_fn("ceil", |x: f64| Ok(x.ceil()));
    math_mod.set_native_fn("round", |x: f64| Ok(x.round()));
    math_mod.set_native_fn("sqrt", |x: f64| Ok(x.sqrt()));
    math_mod.set_native_fn("pow", |base: f64, exp: f64| Ok(base.powf(exp)));
    math_mod.set_native_fn("abs", |x: i64| Ok(x.abs()));
    math_mod.set_native_fn("min", |a: i64, b: i64| Ok(a.min(b)));
    math_mod.set_native_fn("max", |a: i64, b: i64| Ok(a.max(b)));
    engine.register_static_module("math", math_mod.into());
}
