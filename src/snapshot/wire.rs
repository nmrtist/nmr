//! Version-one transport values, independent of runtime model layouts.
use super::{SnapshotError, SnapshotLimits};
use crate::{Complex64, ExecutionContext};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    hash::Hash,
    sync::Arc,
};

/// Private DTO. Numeric vectors are streamed as binary chunks, never text floats.
pub(crate) enum Value<'a> {
    Record(Cow<'a, str>, Vec<Value<'a>>),
    Sequence(Vec<Value<'a>>),
    Text(Cow<'a, str>),
    Unsigned(u64),
    Signed(i64),
    Float(u64),
    Boolean(bool),
    Bytes(Cow<'a, [u8]>),
    Floats(Cow<'a, [f64]>),
    Complexes(Cow<'a, [Complex64]>),
}

pub(crate) struct Budget<'a, 'ctx> {
    pub control: &'a mut ExecutionContext<'ctx>,
    pub limits: SnapshotLimits,
    metadata: usize,
    samples: usize,
}
impl<'a, 'ctx> Budget<'a, 'ctx> {
    pub fn new(control: &'a mut ExecutionContext<'ctx>, limits: SnapshotLimits) -> Self {
        Self {
            control,
            limits,
            metadata: 0,
            samples: 0,
        }
    }
    pub fn reserve<T>(&mut self, count: usize) -> Result<(), SnapshotError> {
        self.control.check_cancelled()?;
        let bytes = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(SnapshotError::SizeOverflow)?;
        self.metadata = self
            .metadata
            .checked_add(bytes)
            .ok_or(SnapshotError::SizeOverflow)?;
        self.check()
    }
    pub fn samples(&mut self, bytes: usize) -> Result<(), SnapshotError> {
        self.control.check_cancelled()?;
        self.samples = self
            .samples
            .checked_add(bytes)
            .ok_or(SnapshotError::SizeOverflow)?;
        self.check()
    }
    fn check(&mut self) -> Result<(), SnapshotError> {
        let total = self
            .metadata
            .checked_add(self.samples)
            .ok_or(SnapshotError::SizeOverflow)?;
        if self.metadata > self.limits.max_metadata_bytes
            || self.samples > self.limits.max_sample_bytes
            || total > self.limits.max_working_bytes
        {
            return Err(SnapshotError::ResourceLimit);
        }
        self.control.observe_payload(total);
        Ok(())
    }
}

/// Explicit conversions through a private versioned DTO; no public model is deserializable.
pub(crate) trait Codec: Sized {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError>;
    fn from_wire(value: Value<'static>, budget: &mut Budget<'_, '_>)
    -> Result<Self, SnapshotError>;
    fn slice_to_wire<'a>(
        values: &'a [Self],
        budget: &mut Budget<'_, '_>,
    ) -> Result<Value<'a>, SnapshotError> {
        budget.reserve::<Value<'_>>(values.len())?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(values.len())
            .map_err(|_| SnapshotError::Allocation)?;
        for value in values {
            output.push(value.to_wire(budget)?);
        }
        Ok(Value::Sequence(output))
    }
    fn slice_from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Vec<Self>, SnapshotError> {
        let Value::Sequence(values) = value else {
            return Err(SnapshotError::Structure);
        };
        budget.reserve::<Self>(values.len())?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(values.len())
            .map_err(|_| SnapshotError::Allocation)?;
        for value in values {
            output.push(Self::from_wire(value, budget)?);
        }
        Ok(output)
    }
}

pub(crate) fn record<'a>(
    tag: &'static str,
    fields: Vec<Value<'a>>,
    budget: &mut Budget<'_, '_>,
) -> Result<Value<'a>, SnapshotError> {
    budget.reserve::<Value<'_>>(fields.len())?;
    Ok(Value::Record(Cow::Borrowed(tag), fields))
}
pub(crate) fn fields(
    value: Value<'static>,
    tag: &str,
    count: usize,
) -> Result<std::vec::IntoIter<Value<'static>>, SnapshotError> {
    match value {
        Value::Record(found, fields) if found == tag && fields.len() == count => {
            Ok(fields.into_iter())
        }
        _ => Err(SnapshotError::Structure),
    }
}
pub(crate) fn next<T: Codec>(
    fields: &mut impl Iterator<Item = Value<'static>>,
    budget: &mut Budget<'_, '_>,
) -> Result<T, SnapshotError> {
    T::from_wire(fields.next().ok_or(SnapshotError::Structure)?, budget)
}

macro_rules! integer {
    ($variant:ident; $($ty:ty),*) => { $(impl Codec for $ty {
        fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> { Ok(Value::$variant((*self).try_into().map_err(|_| SnapshotError::SizeOverflow)?)) }
        fn from_wire(value: Value<'static>, _: &mut Budget<'_, '_>) -> Result<Self, SnapshotError> { if let Value::$variant(v) = value { v.try_into().map_err(|_| SnapshotError::SizeOverflow) } else { Err(SnapshotError::Structure) } }
    })* };
}
integer!(Unsigned; u16, u32, u64, usize);
integer!(Signed; i8, i16, i32, i64, isize);
impl Codec for u128 {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        record(
            "u128.v1",
            vec![
                Value::Unsigned(*self as u64),
                Value::Unsigned((*self >> 64) as u64),
            ],
            budget,
        )
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let mut f = fields(value, "u128.v1", 2)?;
        let low: u64 = next(&mut f, budget)?;
        let high: u64 = next(&mut f, budget)?;
        Ok(u128::from(low) | (u128::from(high) << 64))
    }
}
impl Codec for std::num::NonZeroUsize {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Unsigned(self.get() as u64))
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        Self::new(usize::from_wire(value, budget)?).ok_or(SnapshotError::Structure)
    }
}
impl Codec for u8 {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Unsigned(u64::from(*self)))
    }
    fn from_wire(value: Value<'static>, _: &mut Budget<'_, '_>) -> Result<Self, SnapshotError> {
        if let Value::Unsigned(v) = value {
            v.try_into().map_err(|_| SnapshotError::Structure)
        } else {
            Err(SnapshotError::Structure)
        }
    }
    fn slice_to_wire<'a>(
        values: &'a [Self],
        _: &mut Budget<'_, '_>,
    ) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Bytes(Cow::Borrowed(values)))
    }
    fn slice_from_wire(
        value: Value<'static>,
        _: &mut Budget<'_, '_>,
    ) -> Result<Vec<Self>, SnapshotError> {
        if let Value::Bytes(v) = value {
            Ok(v.into_owned())
        } else {
            Err(SnapshotError::Structure)
        }
    }
}
impl Codec for f64 {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Float(self.to_bits()))
    }
    fn from_wire(value: Value<'static>, _: &mut Budget<'_, '_>) -> Result<Self, SnapshotError> {
        if let Value::Float(v) = value {
            Ok(Self::from_bits(v))
        } else {
            Err(SnapshotError::Structure)
        }
    }
    fn slice_to_wire<'a>(
        values: &'a [Self],
        budget: &mut Budget<'_, '_>,
    ) -> Result<Value<'a>, SnapshotError> {
        budget.samples(
            values
                .len()
                .checked_mul(8)
                .ok_or(SnapshotError::SizeOverflow)?,
        )?;
        Ok(Value::Floats(Cow::Borrowed(values)))
    }
    fn slice_from_wire(
        value: Value<'static>,
        _: &mut Budget<'_, '_>,
    ) -> Result<Vec<Self>, SnapshotError> {
        if let Value::Floats(v) = value {
            Ok(v.into_owned())
        } else {
            Err(SnapshotError::Structure)
        }
    }
}
impl Codec for f32 {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Unsigned(u64::from(self.to_bits())))
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        Ok(Self::from_bits(u32::from_wire(value, budget)?))
    }
}
impl Codec for bool {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Boolean(*self))
    }
    fn from_wire(value: Value<'static>, _: &mut Budget<'_, '_>) -> Result<Self, SnapshotError> {
        if let Value::Boolean(v) = value {
            Ok(v)
        } else {
            Err(SnapshotError::Structure)
        }
    }
}
impl Codec for String {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Text(Cow::Borrowed(self)))
    }
    fn from_wire(value: Value<'static>, _: &mut Budget<'_, '_>) -> Result<Self, SnapshotError> {
        if let Value::Text(v) = value {
            Ok(v.into_owned())
        } else {
            Err(SnapshotError::Structure)
        }
    }
}
impl Codec for &'static str {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Text(Cow::Borrowed(self)))
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let value = String::from_wire(value, budget)?;
        super::registered::lookup(&value).ok_or(SnapshotError::UnsupportedHistoryVersion(value))
    }
}
impl<T: Codec> Codec for Vec<T> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        T::slice_to_wire(self, budget)
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        T::slice_from_wire(value, budget)
    }
}
impl<T: Codec> Codec for Option<T> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        record(
            "option.v1",
            match self {
                Some(v) => vec![v.to_wire(budget)?],
                None => vec![],
            },
            budget,
        )
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        match value {
            Value::Record(tag, mut v) if tag == "option.v1" && v.len() <= 1 => {
                v.pop().map(|v| T::from_wire(v, budget)).transpose()
            }
            _ => Err(SnapshotError::Structure),
        }
    }
}
impl<T: Codec> Codec for Box<T> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        (**self).to_wire(budget)
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        budget.reserve::<T>(1)?;
        Ok(Box::new(T::from_wire(value, budget)?))
    }
}
impl<T: Codec> Codec for Box<[T]> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        T::slice_to_wire(self, budget)
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        Ok(T::slice_from_wire(value, budget)?.into_boxed_slice())
    }
}
impl<T: Codec> Codec for Arc<T> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        (**self).to_wire(budget)
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        budget.reserve::<T>(1)?;
        Ok(Arc::new(T::from_wire(value, budget)?))
    }
}
impl<T: Codec> Codec for Arc<[T]> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        T::slice_to_wire(self, budget)
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let values = T::slice_from_wire(value, budget)?;
        budget.reserve::<T>(values.len())?;
        Ok(values.into())
    }
}
impl Codec for Arc<str> {
    fn to_wire<'a>(&'a self, _: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        Ok(Value::Text(Cow::Borrowed(self)))
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let value = String::from_wire(value, budget)?;
        budget.reserve::<u8>(value.len())?;
        Ok(value.into())
    }
}
impl<T: Codec, const N: usize> Codec for [T; N] {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        T::slice_to_wire(self, budget)
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        T::slice_from_wire(value, budget)?
            .try_into()
            .map_err(|_| SnapshotError::Structure)
    }
}
impl<A: Codec, B: Codec> Codec for (A, B) {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        record(
            "pair.v1",
            vec![self.0.to_wire(budget)?, self.1.to_wire(budget)?],
            budget,
        )
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let mut f = fields(value, "pair.v1", 2)?;
        Ok((next(&mut f, budget)?, next(&mut f, budget)?))
    }
}
impl Codec for Complex64 {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        record(
            "complex64.v1",
            vec![self.re.to_wire(budget)?, self.im.to_wire(budget)?],
            budget,
        )
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let mut f = fields(value, "complex64.v1", 2)?;
        Ok(Self::new(next(&mut f, budget)?, next(&mut f, budget)?))
    }
    fn slice_to_wire<'a>(
        values: &'a [Self],
        budget: &mut Budget<'_, '_>,
    ) -> Result<Value<'a>, SnapshotError> {
        budget.samples(
            values
                .len()
                .checked_mul(16)
                .ok_or(SnapshotError::SizeOverflow)?,
        )?;
        Ok(Value::Complexes(Cow::Borrowed(values)))
    }
    fn slice_from_wire(
        value: Value<'static>,
        _: &mut Budget<'_, '_>,
    ) -> Result<Vec<Self>, SnapshotError> {
        if let Value::Complexes(v) = value {
            Ok(v.into_owned())
        } else {
            Err(SnapshotError::Structure)
        }
    }
}
impl<K: Codec + Ord, V: Codec> Codec for BTreeMap<K, V> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        budget.reserve::<Value<'_>>(self.len())?;
        Ok(Value::Sequence(
            self.iter()
                .map(|(k, v)| {
                    record(
                        "pair.v1",
                        vec![k.to_wire(budget)?, v.to_wire(budget)?],
                        budget,
                    )
                })
                .collect::<Result<_, _>>()?,
        ))
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let pairs = Vec::<(K, V)>::from_wire(value, budget)?;
        let n = pairs.len();
        budget.reserve::<(K, V, [usize; 8])>(n)?;
        let result: Self = pairs.into_iter().collect();
        if result.len() != n {
            return Err(SnapshotError::Structure);
        }
        Ok(result)
    }
}
impl<K: Codec + Eq + Hash + Ord, V: Codec> Codec for HashMap<K, V> {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        budget.reserve::<(&K, &V)>(self.len())?;
        let mut pairs: Vec<_> = self.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        Ok(Value::Sequence(
            pairs
                .into_iter()
                .map(|(k, v)| {
                    record(
                        "pair.v1",
                        vec![k.to_wire(budget)?, v.to_wire(budget)?],
                        budget,
                    )
                })
                .collect::<Result<_, _>>()?,
        ))
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let pairs = Vec::<(K, V)>::from_wire(value, budget)?;
        let n = pairs.len();
        budget.reserve::<(K, V, [usize; 8])>(n)?;
        let result: Self = pairs.into_iter().collect();
        if result.len() != n {
            return Err(SnapshotError::Structure);
        }
        Ok(result)
    }
}
