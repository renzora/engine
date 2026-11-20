#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;

pub use clap::{
    Parser,
    ValueEnum,
};
pub use serde::{
    Deserialize,
    Serialize,
};


#[cfg(feature = "bevy")]
mod plugin;
#[cfg(feature = "bevy")]
pub use plugin::BevyArgsPlugin;


#[cfg(target_arch = "wasm32")]
fn parse_query_string(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let mut iter = pair.splitn(2, '=');
            match (iter.next(), iter.next()) {
                (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                _ => None,
            }
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn update_struct<R>(mut instance: R, query_map: HashMap<String, String>) -> R
where
    R: Serialize + for<'de> Deserialize<'de>,
{
    let mut instance_json = serde_json::to_value(&instance).unwrap();

    for (key, value) in query_map {
        if let Some(field) = instance_json.get_mut(&key) {
            let updated_value = match field {
                serde_json::Value::String(_) => serde_json::Value::String(value.clone()),
                serde_json::Value::Number(_) => {
                    if let Ok(num) = value.parse::<i64>() {
                        serde_json::Value::Number(num.into())
                    } else if let Ok(num) = value.parse::<f64>() {
                        serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap())
                    } else {
                        serde_json::Value::String(value.clone())
                    }
                }
                serde_json::Value::Bool(_) => {
                    if let Ok(b) = value.parse::<bool>() {
                        serde_json::Value::Bool(b)
                    } else {
                        serde_json::Value::String(value.clone())
                    }
                }
                serde_json::Value::Null => serde_json::Value::String(value.clone()),
                _ => serde_json::Value::String(value.clone()), // Handle other types as strings for simplicity
            };
            *field = updated_value;
        }
    }

    instance = serde_json::from_value(instance_json).unwrap();
    instance
}


pub fn parse_args<R: Parser + Serialize + for<'a> Deserialize<'a>>() -> R {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().unwrap();
        let location = window.location();
        let search = location.search().unwrap();

        let query_string = search.trim_start_matches('?');
        let query_map = parse_query_string(query_string);

        let default = R::parse();
        update_struct(default, query_map)
    }

    #[cfg(not(target_arch = "wasm32"))]
    R::parse()
}
