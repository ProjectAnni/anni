use std::io::{Read, Write};

pub trait Decode: Sized {
    type Err;

    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Self::Err>;
}

pub trait Encode {
    type Err;

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Self::Err>;
}
