//! Utilities for migrating data in stable storage across upgrades

use std::cell::RefCell;

use num_traits::AsPrimitive;

thread_local! {
    static STORED_SCHEMA_VERSION: RefCell<u64> = RefCell::default();
}

#[inline]
pub fn set_stored_schema_version(version: u64) {
    STORED_SCHEMA_VERSION.with(|s| *s.borrow_mut() = version);
}

#[inline]
pub fn get_stored_schema_version() -> u64 {
    STORED_SCHEMA_VERSION.with(|s| *s.borrow())
}

/// Deserialize to a default value if the stored schema version is less than `VERSION`.
/// This method is used for one-version forward compatibility for bincode
/// deserialization, and allows adding new fields to the end of structures.
#[inline]
pub fn deserialize_default_if_lt_version<'de, const VERSION: u64, const DEBUG_TAG: u32, T, D>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    let ret = if get_stored_schema_version() < VERSION {
        Ok(T::default())
    } else {
        T::deserialize(deserializer)
    };
    ret.map_err(|e| {
        serde::de::Error::custom(format!("Migration error. Tag {DEBUG_TAG} Error: {e}"))
    })
}

#[inline]
pub fn deserialize_default_if_gte_version<'de, const VERSION: u64, const DEBUG_TAG: u32, T, D>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    let ret = if get_stored_schema_version() >= VERSION {
        Ok(T::default())
    } else {
        T::deserialize(deserializer)
    };
    ret.map_err(|e| {
        serde::de::Error::custom(format!("Migration error. Tag {DEBUG_TAG} Error: {e}"))
    })
}

/// Deserialize using one method if the stored schema version is less than `VERSION`.
/// or another method if the stored version is greater than or equal to `VERSION`.
#[inline]
pub fn deserialize_if_lt_version_or_else<
    'de,
    const VERSION: u64,
    const DEBUG_TAG: u32,
    T,
    D,
    PreVersionFunc,
    PostVersionFunc,
>(
    deserializer: D,
    pre_version_func: PreVersionFunc,
    post_version_func: PostVersionFunc,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
    PreVersionFunc: Fn(D) -> Result<T, D::Error>,
    PostVersionFunc: Fn(D) -> Result<T, D::Error>,
{
    let ret = if get_stored_schema_version() < VERSION {
        pre_version_func(deserializer)
    } else {
        post_version_func(deserializer)
    };
    ret.map_err(|e| {
        serde::de::Error::custom(format!("Migration error. Tag {DEBUG_TAG} Error: {e}"))
    })
}

/// Deserialize into a different vector of primitive type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_into_primitive_collection_if_lt_version<
    'de,
    const VERSION: u64,
    const DEBUG_TAG: u32,
    O,
    T,
    CO,
    CT,
    D,
>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Copy + 'static,
    O: serde::Deserialize<'de> + AsPrimitive<T>,
    CO: IntoIterator<Item = O> + serde::Deserialize<'de>,
    CT: FromIterator<T> + serde::Deserialize<'de>,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let os: CO = serde::Deserialize::deserialize(deserializer)?;
            Ok(os.into_iter().map(|i| i.as_()).collect())
        },
        serde::Deserialize::deserialize,
    )
}

/// Deserialize into a option of a different type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_into_primitive_option_if_lt_version<
    'de,
    const VERSION: u64,
    const DEBUG_TAG: u32,
    O,
    T,
    D,
>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Copy + 'static,
    O: serde::Deserialize<'de> + AsPrimitive<T>,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let o: Option<O> = serde::Deserialize::deserialize(deserializer)?;
            Ok(o.map(|i| i.as_()))
        },
        serde::Deserialize::deserialize,
    )
}

/// Deserialize into a different different type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_into_primitive_if_lt_version<
    'de,
    const VERSION: u64,
    const DEBUG_TAG: u32,
    O,
    T,
    D,
>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Copy + 'static,
    O: serde::Deserialize<'de> + AsPrimitive<T>,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let p: O = serde::Deserialize::deserialize(deserializer)?;
            Ok(p.as_())
        },
        |deserializer| serde::Deserialize::deserialize(deserializer),
    )
}

/// Deserialize into a different vector of primitive type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_into_collection_if_lt_version<
    'de,
    const VERSION: u64,
    const DEBUG_TAG: u32,
    O,
    T,
    CO,
    CT,
    D,
>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
    O: serde::Deserialize<'de> + Into<T>,
    CO: IntoIterator<Item = O> + serde::Deserialize<'de>,
    CT: FromIterator<T> + serde::Deserialize<'de>,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let os: CO = serde::Deserialize::deserialize(deserializer)?;
            Ok(os.into_iter().map(|i| i.into()).collect())
        },
        |deserializer| serde::Deserialize::deserialize(deserializer),
    )
}

/// derialize into a different type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_into_if_lt_version<'de, const VERSION: u64, const DEBUG_TAG: u32, O, T, D>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
    O: serde::Deserialize<'de> + Into<T>,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let o: O = serde::Deserialize::deserialize(deserializer)?;
            Ok(o.into())
        },
        serde::Deserialize::deserialize,
    )
}

/// derialize into a different type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_try_into_if_lt_version<'de, const VERSION: u64, const DEBUG_TAG: u32, O, T, D>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
    O: serde::Deserialize<'de> + TryInto<T>,
    <O as TryInto<T>>::Error: std::fmt::Display,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let o: O = serde::Deserialize::deserialize(deserializer)?;
            o.try_into().map_err(serde::de::Error::custom)
        },
        serde::Deserialize::deserialize,
    )
}

#[inline]
pub fn deserialize_with_debug_tag<'de, const DEBUG_TAG: u32, T, D>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map_err(|e| {
        serde::de::Error::custom(format!("Migration error. Tag {DEBUG_TAG} Error: {e}"))
    })
}

/// Deserialize into a different vector of primitive type if the stored schema version is less than `VERSION`.
#[inline]
pub fn deserialize_into_string_collection_if_lt_version<
    'de,
    const VERSION: u64,
    const DEBUG_TAG: u32,
    O,
    CO,
    D,
>(
    deserializer: D,
) -> Result<Vec<std::string::String>, D::Error>
where
    D: serde::Deserializer<'de>,
    // T: serde::Deserialize<'de> + Copy + 'static,
    O: serde::Deserialize<'de> + ToString,
    CO: IntoIterator<Item = O> + serde::Deserialize<'de>,
    // CT: FromIterator<T> + serde::Deserialize<'de>,
{
    deserialize_if_lt_version_or_else::<'de, VERSION, DEBUG_TAG, _, _, _, _>(
        deserializer,
        |deserializer| {
            let os: CO = serde::Deserialize::deserialize(deserializer)?;
            Ok(os.into_iter().map(|i| i.to_string()).collect())
        },
        serde::Deserialize::deserialize,
    )
}
