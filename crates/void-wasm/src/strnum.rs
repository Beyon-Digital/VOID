//! String-int64 serde helper — package-boundary convention: u64
//! fields cross as decimal strings in JSON, native ints in binary.
//! Mirrors the protocol DTO rule (CONTRACTS.md §1).

pub mod u64str {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        // Accept both the canonical string form and a bare JSON number
        // (defensive at the boundary; canonical output stays a string).
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Str(String),
            Num(u64),
        }
        match Repr::deserialize(d)? {
            Repr::Str(s) => s
                .parse::<u64>()
                .map_err(|_| serde::de::Error::custom(format!("invalid u64 string `{s}`"))),
            Repr::Num(n) => Ok(n),
        }
    }
}
