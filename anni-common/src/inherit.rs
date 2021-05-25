use std::rc::Rc;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::Visitor;
use std::fmt::Formatter;
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone)]
pub enum InheritableValue<T> {
    Owned(Rc<T>),
    Inherited(Option<Rc<T>>),
}

impl<T> InheritableValue<T> {
    pub fn own(value: T) -> Self {
        InheritableValue::Owned(Rc::new(value))
    }

    pub fn inherit(value: Rc<T>) -> Self {
        InheritableValue::Inherited(Some(value))
    }

    pub fn new() -> Self {
        InheritableValue::Inherited(None)
    }

    pub fn get(&self) -> Rc<T> {
        match self {
            InheritableValue::Owned(me) => me.clone(),
            InheritableValue::Inherited(Some(me)) => me.clone(),
            InheritableValue::Inherited(None) => unreachable!(),
        }
    }

    pub fn inherit_from(&mut self, value: &InheritableValue<T>) {
        match self {
            InheritableValue::Inherited(None) => *self = InheritableValue::Inherited(Some(value.get())),
            InheritableValue::Inherited(Some(_)) => panic!("double inherit!"),
            InheritableValue::Owned(_) => {}
        }
    }
}

impl<T> AsRef<T> for InheritableValue<T> {
    fn as_ref(&self) -> &T {
        match self {
            InheritableValue::Owned(me) => me.as_ref(),
            InheritableValue::Inherited(Some(me)) => me.as_ref(),
            InheritableValue::Inherited(None) => unreachable!(),
        }
    }
}

impl<T> InheritableValue<T>
    where T: Clone {
    pub fn inherit_from_owned(&mut self, value: &T) {
        match self {
            InheritableValue::Inherited(None) => *self = InheritableValue::inherit(Rc::new(value.clone())),
            InheritableValue::Inherited(Some(_)) => panic!("double inherit!"),
            InheritableValue::Owned(_) => {}
        }
    }

    pub fn get_raw(&self) -> T {
        match self {
            InheritableValue::Owned(me) => (**me).clone(),
            InheritableValue::Inherited(Some(me)) => (**me).clone(),
            InheritableValue::Inherited(None) => unreachable!(),
        }
    }
}

impl<T> Serialize for InheritableValue<T>
    where
        T: Serialize, {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> where
        S: Serializer {
        match *self {
            InheritableValue::Owned(ref value) => serializer.serialize_some(value.as_ref()),
            InheritableValue::Inherited(_) => serializer.serialize_none(),
        }
    }
}

impl<'de, T> Deserialize<'de> for InheritableValue<T>
    where
        T: Deserialize<'de>, {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error> where
        D: Deserializer<'de> {
        struct InheritableFieldVisitor<T> {
            marker: PhantomData<T>,
        }
        impl<'de, T> Visitor<'de> for InheritableFieldVisitor<T>
            where T: Deserialize<'de> {
            type Value = InheritableValue<T>;

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                formatter.write_str("inheritable field")
            }

            #[inline]
            fn visit_none<E>(self) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
            {
                Ok(InheritableValue::Inherited(None))
            }

            #[inline]
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
            {
                T::deserialize(deserializer).map(InheritableValue::own)
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
            {
                Ok(InheritableValue::Inherited(None))
            }
        }
        deserializer.deserialize_option(InheritableFieldVisitor {
            marker: PhantomData,
        })
    }
}
