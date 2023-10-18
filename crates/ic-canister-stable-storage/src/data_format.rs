//! Standardized interface to serialize/deserialize from a data format.

use candid::{CandidType, Deserialize};
use instrumented_error::IntoInstrumentedError;
use serde::Serialize;
use std::io::{Read, Write};

/// The format type of the
#[derive(Debug, Copy, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq, derive_more::Display)]
#[repr(u64)]
pub enum DataFormatType {
    /// Unknown
    Unknown = 0,
    /// MsgPack
    MsgPack = 1,
    /// Bincode
    Bincode = 2,
}

impl Default for DataFormatType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl From<u64> for DataFormatType {
    fn from(value: u64) -> Self {
        match value {
            1 => Self::MsgPack,
            2 => Self::Bincode,
            _ => Self::Unknown,
        }
    }
}

impl DataFormatType {
    pub fn serde_deserialize<T, Reader>(&self, reader: Reader) -> Result<T, instrumented_error::Error>
    where
        T: for<'a> Deserialize<'a>,
        Reader: Read,
    {
        match self {
            Self::Bincode => Ok(BincodeAdapter::deserialize(reader)?),
            Self::MsgPack => Ok(MsgPackAdapter::deserialize(reader)?),
            f => Err(format!("Incompatible format {}", f).into_instrumented_error()),
        }
    }

    pub fn serde_deserialize_bytes<T>(&self, bytes: &[u8]) -> Result<T, instrumented_error::Error>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut reader = std::io::Cursor::new(bytes);
        self.serde_deserialize(&mut reader)
    }

    pub fn serde_serialize<T, Writer>(&self, writer: Writer, t: &T) -> Result<(), instrumented_error::Error>
    where
        T: serde::Serialize,
        Writer: Write,
    {
        match self {
            Self::Bincode => Ok(BincodeAdapter::serialize(writer, t)?),
            Self::MsgPack => Ok(MsgPackAdapter::serialize(writer, t)?),
            f => Err(format!("Incompatible format {}", f).into_instrumented_error()),
        }
    }

    pub fn serde_serialize_bytes<T>(&self, t: &T) -> Result<Vec<u8>, instrumented_error::Error>
    where
        T: serde::Serialize,
    {
        let mut bytes = Vec::new();
        self.serde_serialize(&mut bytes, t)?;
        Ok(bytes)
    }
}

/// Adapter that defines a common interface for all serde formats for serialization/deserialization
pub trait SerdeDataFormat {
    /// Error type of the serializer
    type SerializeError;
    /// ERror type of the deserializer
    type DeserializeError;

    /// Serialize using a writer
    fn serialize<W, T>(writer: W, t: &T) -> Result<(), Self::SerializeError>
    where
        W: Write,
        T: serde::Serialize;

    /// Deserialize using a reader
    fn deserialize<R, T>(reader: R) -> Result<T, Self::DeserializeError>
    where
        R: Read,
        T: for<'a> serde::Deserialize<'a>;

    /// The format type
    fn format_type() -> DataFormatType;
}

/// MsgPack adapter
pub struct MsgPackAdapter;

impl SerdeDataFormat for MsgPackAdapter {
    type DeserializeError = rmp_serde::decode::Error;
    type SerializeError = rmp_serde::encode::Error;

    fn serialize<W, T>(writer: W, t: &T) -> Result<(), Self::SerializeError>
    where
        W: Write,
        T: serde::Serialize,
    {
        let mut writer = writer;
        rmp_serde::encode::write(&mut writer, t)
    }

    fn deserialize<R, T>(reader: R) -> Result<T, Self::DeserializeError>
    where
        R: Read,
        T: for<'a> serde::Deserialize<'a>,
    {
        let mut reader = reader;
        rmp_serde::decode::from_read(&mut reader)
    }

    fn format_type() -> DataFormatType {
        DataFormatType::MsgPack
    }
}

/// Bincode adapter
pub struct BincodeAdapter;

impl SerdeDataFormat for BincodeAdapter {
    type DeserializeError = bincode::Error;
    type SerializeError = bincode::Error;

    fn serialize<W, T>(writer: W, t: &T) -> Result<(), Self::SerializeError>
    where
        W: Write,
        T: serde::Serialize,
    {
        bincode::serialize_into(writer, t)
    }

    fn deserialize<R, T>(reader: R) -> Result<T, Self::DeserializeError>
    where
        R: Read,
        T: for<'a> serde::Deserialize<'a>,
    {
        bincode::deserialize_from(reader)
    }

    fn format_type() -> DataFormatType {
        DataFormatType::Bincode
    }
}

#[cfg(test)]
mod test {
    use candid::Deserialize;
    use serde::Serialize;
    use std::{cell::RefCell, collections::HashMap, fmt::Debug, ops::DerefMut};

    use super::super::migration::{
        deserialize_default_if_gte_version, deserialize_default_if_lt_version, set_stored_schema_version,
    };

    use super::{BincodeAdapter, SerdeDataFormat};

    thread_local! {
        static STORED_SCHEMA_VERSION: RefCell<u64> = RefCell::default();
    }

    // when adding new fields for a particular upgrade use this number
    // and then increment it.
    const CURRENT_SCHEMA_VERSION: u64 = 2;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct StateV1 {
        pub field1: Vec<u64>,
        pub field2: String,
        pub map: HashMap<u64, NestedV1>,
        pub e: EnumV1,
        pub to_be_removed: HashMap<u64, u64>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct NestedV1 {
        pub field1: i32,
        pub field2: String,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    enum EnumV1 {
        Value,
        AnotherValue,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct StateV2 {
        pub field1: Vec<u64>,
        pub field2: String,
        pub map: HashMap<u64, NestedV2>,
        pub e: EnumV2,
        #[serde(deserialize_with = "deserialize_default_if_lt_version::<'_, CURRENT_SCHEMA_VERSION, 0, _, _>")]
        pub field3: i64,
        #[serde(
            skip_serializing,
            deserialize_with = "deserialize_default_if_gte_version::<'_, CURRENT_SCHEMA_VERSION, 0, _, _>"
        )]
        pub deprecated: HashMap<u64, u64>,
        #[serde(deserialize_with = "deserialize_default_if_lt_version::<'_, CURRENT_SCHEMA_VERSION, 0,  _, _>")]
        pub new_optional: Option<i64>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct StateV2Clean {
        pub field1: Vec<u64>,
        pub field2: String,
        pub map: HashMap<u64, NestedV2>,
        pub e: EnumV2,
        pub field3: i64,
        pub new_optional: Option<i64>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct StateV2Error {
        pub field1: Vec<u64>,
        pub field2: String,
        pub map: HashMap<u64, NestedV2>,
        pub e: EnumV2,
        #[serde(deserialize_with = "deserialize_default_if_lt_version::<'_, CURRENT_SCHEMA_VERSION, 0, _, _>")]
        pub field3: i64,
        #[serde(default)]
        pub new_optional: Option<i64>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct NestedV2 {
        pub field1: i32,
        pub field2: String,
        #[serde(deserialize_with = "deserialize_default_if_lt_version::<'_, CURRENT_SCHEMA_VERSION, 0, _, _>")]
        pub field3: String,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    enum EnumV2 {
        Value,
        AnotherValue,
        ThirdValue,
    }

    impl From<EnumV2> for EnumV1 {
        fn from(val: EnumV2) -> Self {
            match val {
                EnumV2::AnotherValue => Self::AnotherValue,
                EnumV2::Value => Self::Value,
                _ => unimplemented!(),
            }
        }
    }

    // conversion for testing purposes
    impl From<NestedV2> for NestedV1 {
        fn from(val: NestedV2) -> Self {
            NestedV1 {
                field1: val.field1,
                field2: val.field2,
            }
        }
    }

    // conversion for testing purposes
    impl From<StateV2> for StateV1 {
        fn from(val: StateV2) -> Self {
            Self {
                field1: val.field1,
                field2: val.field2,
                map: val.map.into_iter().map(|(key, val)| (key, val.into())).collect(),
                e: val.e.into(),
                to_be_removed: val.deprecated,
            }
        }
    }

    impl From<StateV2> for StateV2Clean {
        fn from(val: StateV2) -> Self {
            Self {
                field1: val.field1,
                field2: val.field2,
                map: val.map,
                e: val.e,
                field3: val.field3,
                new_optional: val.new_optional,
            }
        }
    }

    fn test_format_transition<Adapter>()
    where
        Adapter: SerdeDataFormat,
        <Adapter as SerdeDataFormat>::SerializeError: Debug,
        <Adapter as SerdeDataFormat>::DeserializeError: Debug,
    {
        let v1 = StateV1 {
            field1: vec![10, 20, 30],
            field2: "hello".to_owned(),
            map: HashMap::from([
                (
                    10,
                    NestedV1 {
                        field1: 10,
                        field2: "20".to_owned(),
                    },
                ),
                (
                    20,
                    NestedV1 {
                        field1: 30,
                        field2: "40".to_owned(),
                    },
                ),
            ]),
            e: EnumV1::Value,
            to_be_removed: HashMap::from([(10, 20), (30, 40)]),
        };

        // confirm we can do v1 round trip
        let file = RefCell::new(Vec::<u8>::new());
        Adapter::serialize(file.borrow_mut().deref_mut(), &v1).unwrap();
        let v1_roundtrip: StateV1 = Adapter::deserialize(file.borrow().as_slice()).unwrap();
        assert_eq!(v1, v1_roundtrip);
        set_stored_schema_version(1);

        // deserialize form v1, and ensure the v1 portion of it matches and v2 is initialize to defaults
        let v2_roundtrip: StateV2 = Adapter::deserialize(file.borrow().as_slice()).unwrap();
        assert_eq!(v2_roundtrip.field3, 0);
        assert_eq!(v2_roundtrip.map.get(&10).unwrap().field3, "");
        let v2_v1_roundtrip: StateV1 = v2_roundtrip.clone().into();
        assert_eq!(v2_v1_roundtrip, v1);

        // round trip for v2
        let mut v2 = v2_roundtrip;
        v2.field3 = 10;
        v2.map.insert(
            100,
            NestedV2 {
                field1: 100,
                field2: "1000".to_owned(),
                field3: "actual value".to_owned(),
            },
        );
        v2.e = EnumV2::ThirdValue;
        set_stored_schema_version(CURRENT_SCHEMA_VERSION);
        let file = RefCell::new(Vec::<u8>::new());
        Adapter::serialize(file.borrow_mut().deref_mut(), &v2).unwrap();
        let v2_roundtrip: StateV2 = Adapter::deserialize(file.borrow().as_slice()).unwrap();
        assert_ne!(v2_roundtrip, v2);
        v2.deprecated.clear();
        assert_eq!(v2_roundtrip, v2);

        // another round trip serialization to make sure we deserialize the skipped field correctly
        let file = RefCell::new(Vec::<u8>::new());
        Adapter::serialize(file.borrow_mut().deref_mut(), &v2_roundtrip).unwrap();
        let v2_roundtrip: StateV2 = Adapter::deserialize(file.borrow().as_slice()).unwrap();
        assert_eq!(v2_roundtrip, v2);

        // ensure we can deserialize into a struct without the field
        let v2_clean: StateV2Clean = Adapter::deserialize(file.borrow().as_slice()).unwrap();
        let v2_orig: StateV2Clean = v2_roundtrip.into();
        assert_eq!(v2_orig, v2_clean);
    }

    // disabling msgpack support, since it doesn't work with serialize_skip
    // https://github.com/3Hren/msgpack-rust/issues/86
    // #[test]
    // fn test_msgpack_transition() {
    //     use super::MsgPackAdapter;
    //     test_format_transition::<MsgPackAdapter>();
    // }

    #[test]
    fn test_bincode_transition() {
        use super::BincodeAdapter;
        // bincode does not handle serializing back to a struct with new fields
        test_format_transition::<BincodeAdapter>();
    }

    #[test]
    fn test_bincode_error() {
        let v1 = StateV1 {
            field1: vec![10, 20, 30],
            field2: "hello".to_owned(),
            map: HashMap::from([
                (
                    10,
                    NestedV1 {
                        field1: 10,
                        field2: "20".to_owned(),
                    },
                ),
                (
                    20,
                    NestedV1 {
                        field1: 30,
                        field2: "40".to_owned(),
                    },
                ),
            ]),
            e: EnumV1::Value,
            to_be_removed: HashMap::from([(10, 20), (30, 40)]),
        };

        // confirm we can do v1 round trip
        let file = RefCell::new(Vec::<u8>::new());
        BincodeAdapter::serialize(file.borrow_mut().deref_mut(), &v1).unwrap();
        let v1_roundtrip: StateV1 = BincodeAdapter::deserialize(file.borrow().as_slice()).unwrap();
        assert_eq!(v1, v1_roundtrip);
        set_stored_schema_version(1);

        // deserialize form v1, and ensure the v1 portion of it matches and v2 is initialize to defaults
        let _: StateV2 = BincodeAdapter::deserialize(file.borrow().as_slice()).unwrap();
        // just annotating with serde(default) errors
        let ret: Result<StateV2Error, _> = BincodeAdapter::deserialize(file.borrow().as_slice());
        assert!(ret.is_err());
    }
}
