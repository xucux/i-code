//! # JSON Host Functions
//!
//! `json.parse` / `json.stringify` / `json.stringify_pretty`

use rhai::{Dynamic, Engine, Map};

/// 注册 JSON host 函数
///
/// - 扁平名：`json_parse` / `json_stringify` / `json_stringify_pretty`
/// - 静态模块：`json::parse` / `json::stringify` / `json::stringify_pretty`
///
/// 注意：模块调用请用 `json::parse(...)`，不要写 `json.parse(...)`。
pub fn register(engine: &mut Engine) {
    engine.register_fn("json_parse", |text: &str| -> Result<Dynamic, Box<rhai::EvalAltResult>> {
        parse_json(text)
    });
    engine.register_fn(
        "json_stringify",
        |value: Dynamic| -> Result<String, Box<rhai::EvalAltResult>> {
            let v = dynamic_to_serde(&value)?;
            serde_json::to_string(&v).map_err(|e| format!("JSON 序列化失败: {e}").into())
        },
    );
    engine.register_fn(
        "json_stringify_pretty",
        |value: Dynamic| -> Result<String, Box<rhai::EvalAltResult>> {
            let v = dynamic_to_serde(&value)?;
            serde_json::to_string_pretty(&v).map_err(|e| format!("JSON 序列化失败: {e}").into())
        },
    );

    let mut module = rhai::Module::new();
    module.set_native_fn("parse", |text: &str| parse_json(text));
    module.set_native_fn("stringify", |value: Dynamic| {
        let v = dynamic_to_serde(&value)?;
        serde_json::to_string(&v).map_err(|e| format!("JSON 序列化失败: {e}").into())
    });
    module.set_native_fn("stringify_pretty", |value: Dynamic| {
        let v = dynamic_to_serde(&value)?;
        serde_json::to_string_pretty(&v).map_err(|e| format!("JSON 序列化失败: {e}").into())
    });
    engine.register_static_module("json", module.into());
}

fn parse_json(text: &str) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("JSON 解析失败: {e}"))?;
    Ok(serde_to_dynamic(&v))
}

pub(crate) fn serde_to_dynamic(v: &serde_json::Value) -> Dynamic {
    match v {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::from(n.to_string())
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let list: rhai::Array = arr.iter().map(serde_to_dynamic).collect();
            Dynamic::from(list)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Map::new();
            for (k, val) in obj {
                map.insert(k.clone().into(), serde_to_dynamic(val));
            }
            Dynamic::from_map(map)
        }
    }
}

pub(crate) fn dynamic_to_serde(
    d: &Dynamic,
) -> Result<serde_json::Value, Box<rhai::EvalAltResult>> {
    if d.is_unit() {
        return Ok(serde_json::Value::Null);
    }
    if let Some(b) = d.clone().try_cast::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = d.as_int() {
        return Ok(serde_json::json!(i));
    }
    if let Ok(f) = d.as_float() {
        return Ok(serde_json::json!(f));
    }
    if let Ok(s) = d.clone().into_string() {
        return Ok(serde_json::Value::String(s));
    }
    if let Some(arr) = d.clone().try_cast::<rhai::Array>() {
        let mut out = Vec::new();
        for item in arr {
            out.push(dynamic_to_serde(&item)?);
        }
        return Ok(serde_json::Value::Array(out));
    }
    if let Some(map) = d.clone().try_cast::<Map>() {
        let mut obj = serde_json::Map::new();
        for (k, v) in map {
            obj.insert(k.to_string(), dynamic_to_serde(&v)?);
        }
        return Ok(serde_json::Value::Object(obj));
    }
    Ok(serde_json::Value::String(d.to_string()))
}
