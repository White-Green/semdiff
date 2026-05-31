use nom::error::ErrorKind;
use nom::{Err, Mode, Offset, OutputMode, PResult, Parser};
use std::marker::PhantomData;
use std::str::FromStr;

pub(super) struct IntegerLiteral<E>(PhantomData<E>);

impl<E> IntegerLiteral<E> {
    pub(super) fn new() -> Self {
        IntegerLiteral(PhantomData)
    }
}

impl<'a, E> Parser<&'a str> for IntegerLiteral<E>
where
    E: nom::error::ParseError<&'a str>,
{
    type Output = i64;
    type Error = E;

    fn process<OM: OutputMode>(&mut self, input: &'a str) -> PResult<OM, &'a str, Self::Output, Self::Error> {
        let first = input;
        let input = if let Some(tail) = input.strip_prefix("0") {
            tail
        } else {
            let input = input.strip_prefix("-").unwrap_or(input);
            let mut input = input
                .strip_prefix(|c: char| matches!(c, '1'..='9'))
                .ok_or_else(|| Err::Error(OM::Error::bind(|| E::from_error_kind(input, ErrorKind::Digit))))?;
            while let Some(tail) = input.strip_prefix(|c: char| c.is_ascii_digit()) {
                input = tail;
            }
            input
        };

        let integer = &first[..first.offset(input)];
        i64::from_str(integer)
            .map(|n| (input, OM::Output::bind(|| n)))
            .map_err(|_| Err::Error(OM::Error::bind(|| E::from_error_kind(integer, ErrorKind::Digit))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_path::integer_literal::IntegerLiteral;
    use nom::error::Error;

    #[test]
    fn integer_literal_parser_accepts_int_alternatives() {
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("0"), Ok(("", 0)));
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("1"), Ok(("", 1)));
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("9"), Ok(("", 9)));
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("10"), Ok(("", 10)));
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("-1"), Ok(("", -1)));
        assert_eq!(
            IntegerLiteral::<Error<&str>>::new().parse("-9876543210"),
            Ok(("", -9876543210))
        );
    }

    #[test]
    fn integer_literal_parser_returns_remaining_input() {
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("123]"), Ok(("]", 123)));
        assert_eq!(
            IntegerLiteral::<Error<&str>>::new().parse("-1, next"),
            Ok((", next", -1))
        );
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("1.25"), Ok((".25", 1)));
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("1e2"), Ok(("e2", 1)));
        assert_eq!(IntegerLiteral::<Error<&str>>::new().parse("01"), Ok(("1", 0)));
    }

    #[test]
    fn integer_literal_parser_rejects_inputs_without_an_int_prefix() {
        assert!(IntegerLiteral::<Error<&str>>::new().parse("").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse("-").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse("-0").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse("-01").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse("+1").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse(".1").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse("e1").is_err());
        assert!(IntegerLiteral::<Error<&str>>::new().parse("abc").is_err());
    }

    #[test]
    fn integer_literal_parser_rejects_out_of_range_values() {
        assert!(
            IntegerLiteral::<Error<&str>>::new()
                .parse("9223372036854775808")
                .is_err()
        );
        assert!(
            IntegerLiteral::<Error<&str>>::new()
                .parse("-9223372036854775809")
                .is_err()
        );
    }
}
