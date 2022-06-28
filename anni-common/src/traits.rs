use std::io::{Read, Write};

pub trait Decode: Sized {
    type Err;

    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Self::Err>;
}

pub trait Encode {
    type Err;

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Self::Err>;
}

pub trait Handle {
    #[inline(always)]
    fn handle(&self) -> anyhow::Result<()> {
        self.handle_subcommand()
    }

    #[inline(always)]
    fn handle_subcommand(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub trait HandleArgs<T = ()> {
    fn handle(&self, arg: &T) -> anyhow::Result<()>;
}
