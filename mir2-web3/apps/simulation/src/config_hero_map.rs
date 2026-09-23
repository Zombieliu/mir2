//! JSON map decoding must reject duplicate IDs rather than dropping one record.
use super::SharedHeroRecord;
use serde::{de::{Error, MapAccess, Visitor}, Deserializer};
use std::{collections::BTreeMap, fmt};

pub(in crate::config) fn deserialize_records<'de,D:Deserializer<'de>>(deserializer:D)->Result<BTreeMap<i32,SharedHeroRecord>,D::Error>{
    struct Records;
    impl<'de> Visitor<'de> for Records {
        type Value=BTreeMap<i32,SharedHeroRecord>;
        fn expecting(&self,formatter:&mut fmt::Formatter<'_>)->fmt::Result{formatter.write_str("a Hero registry with unique positive Int32 keys")}
        fn visit_map<M:MapAccess<'de>>(self,mut map:M)->Result<Self::Value,M::Error>{
            let mut records=BTreeMap::new();
            while let Some((id,record))=map.next_entry::<i32,SharedHeroRecord>()?{
                if id<=0 || records.insert(id,record).is_some(){return Err(M::Error::custom("invalid or duplicate Hero registry key"));}
            }
            Ok(records)
        }
    }
    deserializer.deserialize_map(Records)
}
