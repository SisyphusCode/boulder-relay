use std::marker::PhantomData;
use std::ops::Deref;

use syn::parse::{Parse, ParseBuffer, ParseStream};

/// Like `Cow` but without the clone requirement
enum PossiblyBorrowed<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}
impl<'a, T> Deref for PossiblyBorrowed<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            PossiblyBorrowed::Borrowed(b) => b,
            PossiblyBorrowed::Owned(ref o) => o,
        }
    }
}

/// [`Punctuated`](syn::punctuated::Punctuated), in iterator form
pub struct DelimitedIter<'a, T, D> {
    parse: PossiblyBorrowed<'a, ParseBuffer<'a>>,
    errored: bool,
    _marker: PhantomData<(T, D)>,
}

impl<'a, T, D> DelimitedIter<'a, T, D> {
    /// Construct an iterator from the stream and delimiter
    pub const fn new(parse: ParseStream<'a>) -> Self {
        Self::construct(PossiblyBorrowed::Borrowed(parse))
    }
    /// Construct an iterator from the buffer and a delimiter
    pub const fn new_buffer(parse: ParseBuffer<'a>) -> Self {
        Self::construct(PossiblyBorrowed::Owned(parse))
    }

    #[inline]
    const fn construct(parse: PossiblyBorrowed<'a, ParseBuffer<'a>>) -> Self {
        Self {
            parse,
            errored: false,
            _marker: PhantomData,
        }
    }

    #[inline]
    #[allow(clippy::unnecessary_wraps)]
    fn on_error(&mut self, err: syn::Error) -> Option<syn::Result<T>> {
        self.errored = true;
        Some(Err(err))
    }
}

impl<'a, T, D> From<ParseStream<'a>> for DelimitedIter<'a, T, D> {
    #[inline]
    fn from(value: ParseStream<'a>) -> Self {
        Self::new(value)
    }
}

impl<'a, T, D> From<ParseBuffer<'a>> for DelimitedIter<'a, T, D> {
    #[inline]
    fn from(value: ParseBuffer<'a>) -> Self {
        Self::new_buffer(value)
    }
}

impl<'a, T: Parse, D: Parse> Iterator for DelimitedIter<'a, T, D> {
    type Item = syn::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.parse.is_empty() || self.errored {
            return None;
        }

        let output = match self.parse.parse::<T>() {
            Ok(o) => o,
            Err(e) => return self.on_error(e),
        };

        if self.parse.is_empty() {
            return Some(Ok(output));
        }

        if let Err(e) = self.parse.parse::<D>() {
            return self.on_error(e);
        }

        Some(Ok(output))
    }
}
