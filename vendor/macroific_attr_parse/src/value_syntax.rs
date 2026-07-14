use syn::parse::{Parse, ParseBuffer, ParseStream};
use syn::{parenthesized, Token};

/// Syntax used for providing a value
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ValueSyntax {
    /// `= contents`
    Eq,

    /// `(contents)`
    Paren,
}

impl ValueSyntax {
    /// Returns `true` if the syntax is [`Eq`](ValueSyntax::Eq).
    #[inline]
    #[must_use]
    pub const fn is_eq(self) -> bool {
        matches!(self, Self::Eq)
    }

    /// Returns `true` if the syntax is [`Paren`](ValueSyntax::Paren).
    #[inline]
    #[must_use]
    pub const fn is_paren(self) -> bool {
        matches!(self, Self::Paren)
    }

    /// Peek the stream without moving the cursor and attempt to construct self based on the next
    /// token
    pub fn from_stream(parse: ParseStream) -> Option<Self> {
        if parse.peek(Token![=]) {
            Some(Self::Eq)
        } else if parse.peek(syn::token::Paren) {
            Some(Self::Paren)
        } else {
            None
        }
    }

    /// Parse whatever tokens need to be parsed based on the resolved syntax.
    /// Returns a `ParseBuffer` you should continue parsing if the syntax is
    /// [`Paren`](ValueSyntax::Paren).
    pub fn parse_token(self, input: ParseStream) -> syn::Result<Option<ParseBuffer>> {
        match self {
            Self::Eq => {
                input.parse::<Token![=]>()?;
                Ok(None)
            }
            Self::Paren => {
                let content;
                parenthesized!(content in input);
                Ok(Some(content))
            }
        }
    }

    /// Parse whatever tokens need to be parsed based on the resolved syntax and
    /// then parse the referenced value as `P`.
    pub fn parse<P: Parse>(self, input: ParseStream) -> syn::Result<P> {
        if let Some(inner) = self.parse_token(input)? {
            inner.parse()
        } else {
            input.parse()
        }
    }
}
