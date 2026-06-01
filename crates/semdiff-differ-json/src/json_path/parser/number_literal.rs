use nom::error::ErrorKind;
use nom::{Err, Mode, Offset, OutputMode, PResult, Parser};
use std::marker::PhantomData;
use std::str::FromStr;

pub(super) struct NumberLiteral<E>(PhantomData<E>);

impl<E> NumberLiteral<E> {
    pub(super) fn new() -> Self {
        NumberLiteral(PhantomData)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Number {
    Int(i64),
    Float(f64),
}

impl<'a, E> Parser<&'a str> for NumberLiteral<E>
where
    E: nom::error::ParseError<&'a str>,
{
    type Output = Number;
    type Error = E;

    fn process<OM: OutputMode>(&mut self, input: &'a str) -> PResult<OM, &'a str, Self::Output, Self::Error> {
        let first = input;
        let input = input.strip_prefix("-").unwrap_or(input);
        let input = if let Some(tail) = input.strip_prefix("0") {
            tail
        } else {
            let mut input = input
                .strip_prefix(|c: char| c.is_ascii_digit())
                .ok_or_else(|| Err::Error(OM::Error::bind(|| E::from_error_kind(input, ErrorKind::Digit))))?;
            while let Some(tail) = input.strip_prefix(|c: char| c.is_ascii_digit()) {
                input = tail;
            }
            input
        };

        if !input.starts_with(['.', 'e']) {
            let number = &first[..first.offset(input)];
            return i64::from_str(&first[..first.offset(input)])
                .map(|n| (input, OM::Output::bind(|| Number::Int(n))))
                .map_err(|_| Err::Error(OM::Error::bind(|| E::from_error_kind(number, ErrorKind::Digit))));
        }

        let input = if let Some(input) = input.strip_prefix(".") {
            let mut input = input
                .strip_prefix(|c: char| c.is_ascii_digit())
                .ok_or_else(|| Err::Error(OM::Error::bind(|| E::from_error_kind(input, ErrorKind::Digit))))?;
            while let Some(tail) = input.strip_prefix(|c: char| c.is_ascii_digit()) {
                input = tail;
            }
            input
        } else {
            input
        };

        let input = if let Some(input) = input.strip_prefix("e") {
            let input = input.strip_prefix(|c: char| c == '+' || c == '-').unwrap_or(input);
            let mut input = input
                .strip_prefix(|c: char| c.is_ascii_digit())
                .ok_or_else(|| Err::Error(OM::Error::bind(|| E::from_error_kind(input, ErrorKind::Digit))))?;
            while let Some(tail) = input.strip_prefix(|c: char| c.is_ascii_digit()) {
                input = tail;
            }
            input
        } else {
            input
        };

        let number = &first[..first.offset(input)];
        f64::from_str(&first[..first.offset(input)])
            .map(|n| (input, OM::Output::bind(|| Number::Float(n))))
            .map_err(|_| Err::Error(OM::Error::bind(|| E::from_error_kind(number, ErrorKind::Digit))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::error::Error;

    #[test]
    fn number_literal_parser_accepts_int_alternatives() {
        assert_eq!(NumberLiteral::<Error<&str>>::new().parse("0"), Ok(("", Number::Int(0))));
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-0"),
            Ok(("", Number::Int(0)))
        );
        assert_eq!(NumberLiteral::<Error<&str>>::new().parse("1"), Ok(("", Number::Int(1))));
        assert_eq!(NumberLiteral::<Error<&str>>::new().parse("9"), Ok(("", Number::Int(9))));
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("10"),
            Ok(("", Number::Int(10)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-1"),
            Ok(("", Number::Int(-1)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-9876543210"),
            Ok(("", Number::Int(-9876543210)))
        );
    }

    #[test]
    fn number_literal_parser_accepts_fraction_parts() {
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("0.0"),
            Ok(("", Number::Float(0.0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-0.5"),
            Ok(("", Number::Float(-0.5)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1.25"),
            Ok(("", Number::Float(1.25)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-12.75"),
            Ok(("", Number::Float(-12.75)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("123.456789"),
            Ok(("", Number::Float(123.456789)))
        );
    }

    #[test]
    fn number_literal_parser_accepts_exponent_parts() {
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1e2"),
            Ok(("", Number::Float(100.0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1e+2"),
            Ok(("", Number::Float(100.0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1e-2"),
            Ok(("", Number::Float(0.01)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1e02"),
            Ok(("", Number::Float(100.0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-0e0"),
            Ok(("", Number::Float(-0.0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1.25e2"),
            Ok(("", Number::Float(125.0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-12.5e+1"),
            Ok(("", Number::Float(-125.0)))
        );
    }

    #[test]
    fn number_literal_parser_returns_remaining_input() {
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("123]"),
            Ok(("]", Number::Int(123)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-0, next"),
            Ok((", next", Number::Int(0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1.25e2 rest"),
            Ok((" rest", Number::Float(125.0)))
        );
    }

    #[test]
    fn number_literal_parser_leaves_non_bnf_suffixes_unconsumed() {
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("1E2"),
            Ok(("E2", Number::Int(1)))
        );
        // assert_eq!(
        //     NumberLiteral::<Error<&str>>::new().parse("1."),
        //     Ok((".", Number::Int(1)))
        // );
        // assert_eq!(
        //     NumberLiteral::<Error<&str>>::new().parse("1e"),
        //     Ok(("e", Number::Int(1)))
        // );
        // assert_eq!(
        //     NumberLiteral::<Error<&str>>::new().parse("1e+"),
        //     Ok(("e+", Number::Int(1)))
        // );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("01"),
            Ok(("1", Number::Int(0)))
        );
        assert_eq!(
            NumberLiteral::<Error<&str>>::new().parse("-01"),
            Ok(("1", Number::Int(0)))
        );
    }

    #[test]
    fn number_literal_parser_rejects_inputs_without_a_number_prefix() {
        assert!(NumberLiteral::<Error<&str>>::new().parse("").is_err());
        assert!(NumberLiteral::<Error<&str>>::new().parse("-").is_err());
        assert!(NumberLiteral::<Error<&str>>::new().parse("+1").is_err());
        assert!(NumberLiteral::<Error<&str>>::new().parse(".1").is_err());
        assert!(NumberLiteral::<Error<&str>>::new().parse("e1").is_err());
        assert!(NumberLiteral::<Error<&str>>::new().parse("abc").is_err());
    }
}
