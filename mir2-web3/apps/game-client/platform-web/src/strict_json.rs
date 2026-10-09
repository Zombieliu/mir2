//! Shared strict JSON visitor; no WebAssembly adapter or exports.
use serde_json::{json, Value};

pub(crate) struct StrictMailValue(pub(crate) Value);
impl<'de> serde::Deserialize<'de> for StrictMailValue {
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error>{
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value=StrictMailValue;
            fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("JSON without duplicate keys")}
            fn visit_bool<E:serde::de::Error>(self,v:bool)->Result<Self::Value,E>{Ok(StrictMailValue(json!(v)))}
            fn visit_i64<E:serde::de::Error>(self,v:i64)->Result<Self::Value,E>{Ok(StrictMailValue(json!(v)))}
            fn visit_u64<E:serde::de::Error>(self,v:u64)->Result<Self::Value,E>{Ok(StrictMailValue(json!(v)))}
            fn visit_f64<E:serde::de::Error>(self,v:f64)->Result<Self::Value,E>{serde_json::Number::from_f64(v).map(|n|StrictMailValue(Value::Number(n))).ok_or_else(||E::custom("invalid number"))}
            fn visit_str<E:serde::de::Error>(self,v:&str)->Result<Self::Value,E>{Ok(StrictMailValue(json!(v)))}
            fn visit_string<E:serde::de::Error>(self,v:String)->Result<Self::Value,E>{Ok(StrictMailValue(json!(v)))}
            fn visit_unit<E:serde::de::Error>(self)->Result<Self::Value,E>{Ok(StrictMailValue(Value::Null))}
            fn visit_none<E:serde::de::Error>(self)->Result<Self::Value,E>{Ok(StrictMailValue(Value::Null))}
            fn visit_seq<A:serde::de::SeqAccess<'de>>(self,mut sequence:A)->Result<Self::Value,A::Error>{
                let mut values=Vec::new();while let Some(StrictMailValue(v))=sequence.next_element()?{values.push(v);}Ok(StrictMailValue(Value::Array(values)))
            }
            fn visit_map<A:serde::de::MapAccess<'de>>(self,mut map:A)->Result<Self::Value,A::Error>{
                let mut values=serde_json::Map::new();while let Some(key)=map.next_key::<String>()?{
                    if values.contains_key(&key){return Err(<A::Error as serde::de::Error>::custom("duplicate field"));}
                    let StrictMailValue(value)=map.next_value()?;values.insert(key,value);
                }Ok(StrictMailValue(Value::Object(values)))
            }
        }deserializer.deserialize_any(Visitor)
    }
}
