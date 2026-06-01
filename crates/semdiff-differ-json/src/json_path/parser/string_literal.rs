use nom::error::ErrorKind;
use nom::{Err, Mode, Needed, OutputMode, Parser};
use std::marker::PhantomData;
use std::num::NonZeroUsize;

pub(super) struct StringLiteral<E>(PhantomData<E>);

impl<E> StringLiteral<E> {
    pub(super) fn new() -> StringLiteral<E> {
        StringLiteral(PhantomData)
    }
}

impl<'a, E> Parser<&'a str> for StringLiteral<E>
where
    E: nom::error::ParseError<&'a str>,
{
    type Output = String;
    type Error = E;

    fn process<OM: OutputMode>(&mut self, input: &'a str) -> nom::PResult<OM, &'a str, Self::Output, Self::Error> {
        let mut chars = input.char_indices();
        macro_rules! take_matches {
            ($pattern:pat, $need:expr) => {
                match chars.next() {
                    None => return Err(Err::Incomplete(Needed::Size(NonZeroUsize::new($need).unwrap()))),
                    #[allow(unused_parens)]
                    Some((_, c @ ($pattern))) => c,
                    Some((i, _)) => {
                        return Err(Err::Error(OM::Error::bind(|| {
                            E::from_error_kind(&input[i..], ErrorKind::Char)
                        })))
                    }
                }
            };
            ($pattern:pat) => {
                match chars.next() {
                    None => return Err(Err::Incomplete(Needed::Unknown)),
                    #[allow(unused_parens)]
                    Some((_, c @ ($pattern))) => c,
                    Some((i, _)) => {
                        return Err(Err::Error(OM::Error::bind(|| {
                            E::from_error_kind(&input[i..], ErrorKind::Char)
                        })))
                    }
                }
            };
        }
        #[inline(always)]
        const fn char2int(c: char) -> u32 {
            match c {
                '0'..='9' => c as u32 - b'0' as u32,
                'A'..='F' => c as u32 - b'A' as u32 + 10,
                _ => unreachable!(),
            }
        }
        macro_rules! parse_literal_tail {
            ($escaped_quote:tt, $allow_quote:tt) => {
                {
                    let mut result = String::new();
                    while let Some((i, c)) = chars.next() {
                        match c {
                            $escaped_quote => return Ok((&input[i + c.len_utf8()..], OM::Output::bind(|| result))),
                            '\\' => {
                                let Some((_, c)) = chars.next() else {
                                    return Err(Err::Incomplete(Needed::Size(NonZeroUsize::new(1).unwrap())));
                                };
                                match c {
                                    $escaped_quote => result.push($escaped_quote),
                                    '\x62' => result.push('\u{0008}'),
                                    '\x66' => result.push('\u{000C}'),
                                    '\x6E' => result.push('\u{000A}'),
                                    '\x72' => result.push('\u{000D}'),
                                    '\x74' => result.push('\u{0009}'),
                                    '/' => result.push('\u{002F}'),
                                    '\\' => result.push('\u{005C}'),
                                    '\x75' => match take_matches!('0'..='9' | 'A'..='F', 4) {
                                        c1 @ 'D' => match take_matches!('0'..='9' | 'A'..='B', 3) {
                                            c2 @ '0'..='7' => {
                                                let c1 = char2int(c1);
                                                let c2 = char2int(c2);
                                                let c3 = char2int(take_matches!('0'..='9' | 'A'..='F', 2));
                                                let c4 = char2int(take_matches!('0'..='9' | 'A'..='F', 1));
                                                result
                                                    .push(char::from_u32((c1 << 12) | (c2 << 8) | (c3 << 4) | c4).unwrap());
                                            }
                                            c2 @ ('8'..='9' | 'A'..='B') => {
                                                let c1 = char2int(c1);
                                                let c2 = char2int(c2);
                                                let c3 = char2int(take_matches!('0'..='9' | 'A'..='F', 8));
                                                let c4 = char2int(take_matches!('0'..='9' | 'A'..='F', 7));
                                                let high_surrogate = ((c1 << 12) | (c2 << 8) | (c3 << 4) | c4) as u16;
                                                take_matches!('\\', 6);
                                                take_matches!('\x75', 5);
                                                let c1 = char2int(take_matches!('D', 4));
                                                let c2 = char2int(take_matches!('C'..='F', 3));
                                                let c3 = char2int(take_matches!('0'..='9' | 'A'..='F', 2));
                                                let c4 = char2int(take_matches!('0'..='9' | 'A'..='F', 1));
                                                let low_surrogate = ((c1 << 12) | (c2 << 8) | (c3 << 4) | c4) as u16;
                                                let mut utf16 = char::decode_utf16([high_surrogate, low_surrogate]);
                                                result.push(utf16.next().unwrap().unwrap());
                                                assert!(utf16.next().is_none());
                                            }
                                            _ => unreachable!(),
                                        },
                                        c1 @ ('0'..='9' | 'A'..='C' | 'E'..='F') => {
                                            let c1 = char2int(c1);
                                            let c2 = char2int(take_matches!('0'..='9' | 'A'..='F', 3));
                                            let c3 = char2int(take_matches!('0'..='9' | 'A'..='F', 2));
                                            let c4 = char2int(take_matches!('0'..='9' | 'A'..='F', 1));
                                            result.push(char::from_u32((c1 << 12) | (c2 << 8) | (c3 << 4) | c4).unwrap());
                                        }
                                        _ => unreachable!(),
                                    },
                                    _ => return Err(Err::Error(OM::Error::bind(|| E::from_error_kind(&input[i..], ErrorKind::Char)))),
                                }
                            }
                            c @ ('\x20'..='\x21'
                            | $allow_quote
                            | '\x23'..='\x26'
                            | '\x28'..='\x5B'
                            | '\x5D'..='\u{D7FF}'
                            | '\u{E000}'..='\u{10FFFF}') => result.push(c),
                            _ => return Err(Err::Error(OM::Error::bind(|| E::from_error_kind(&input[i..], ErrorKind::Char)))),
                        }
                    }
                    return Err(Err::Incomplete(Needed::Size(NonZeroUsize::new(1).unwrap())));
                }
            }
        }
        match chars.next() {
            Some((_, '"')) => parse_literal_tail!('"', '\''),
            Some((_, '\'')) => parse_literal_tail!('\'', '"'),
            None => Err(nom::Err::Incomplete(Needed::Size(NonZeroUsize::new(2).unwrap()))),
            Some((_, _)) => Err(nom::Err::Error(OM::Error::bind(|| {
                E::from_error_kind(input, ErrorKind::Char)
            }))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::error::Error;

    #[test]
    fn string_literal_parser_accepts_empty_literals_and_quote_variants() {
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse(r#""""#),
            Ok(("", "".to_owned()))
        );
        assert_eq!(StringLiteral::<Error<&str>>::new().parse("''"), Ok(("", "".to_owned())));
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse(r#""single quote: '""#),
            Ok(("", "single quote: '".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("'double quote: \"'"),
            Ok(("", "double quote: \"".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\\"\""),
            Ok(("", "\"".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("'\\''"),
            Ok(("", "'".to_owned()))
        );
    }

    #[test]
    fn string_literal_parser_accepts_unescaped_bnf_ranges() {
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\" \u{0021}\""),
            Ok(("", " \u{0021}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"#$%&\""),
            Ok(("", "#$%&".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"()*+,-./09:;<=>?@AZ[\""),
            Ok(("", "()*+,-./09:;<=>?@AZ[".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\u{005D}\u{007F}\u{0080}\u{D7FF}\u{E000}\u{10FFFF}\""),
            Ok(("", "\u{005D}\u{007F}\u{0080}\u{D7FF}\u{E000}\u{10FFFF}".to_owned())),
        );
    }

    #[test]
    fn string_literal_parser_accepts_escapable_chars() {
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\b\\f\\n\\r\\t\\/\\\\\""),
            Ok(("", "\u{0008}\u{000C}\n\r\t/\\".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("'\\b\\f\\n\\r\\t\\/\\\\'"),
            Ok(("", "\u{0008}\u{000C}\n\r\t/\\".to_owned()))
        );
    }

    #[test]
    fn string_literal_parser_accepts_non_surrogate_unicode_escapes() {
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\u0000\""),
            Ok(("", "\u{0000}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\u000B\""),
            Ok(("", "\u{000B}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\u0041\""),
            Ok(("", "A".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uA000\""),
            Ok(("", "\u{A000}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uBEEF\""),
            Ok(("", "\u{BEEF}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uCDEF\""),
            Ok(("", "\u{CDEF}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uD7FF\""),
            Ok(("", "\u{D7FF}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uE000\""),
            Ok(("", "\u{E000}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uFFFF\""),
            Ok(("", "\u{FFFF}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("'\\u0061'"),
            Ok(("", "a".to_owned()))
        );
    }

    #[test]
    fn string_literal_parser_accepts_surrogate_pair_unicode_escapes() {
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uD800\\uDC00\""),
            Ok(("", "\u{10000}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uD83D\\uDE00\""),
            Ok(("", "\u{1F600}".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("\"\\uDBFF\\uDFFF\""),
            Ok(("", "\u{10FFFF}".to_owned()))
        );
    }

    #[test]
    fn string_literal_parser_returns_remaining_input() {
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse(r#""member"]"#),
            Ok(("]", "member".to_owned()))
        );
        assert_eq!(
            StringLiteral::<Error<&str>>::new().parse("'member', next"),
            Ok((", next", "member".to_owned()))
        );
    }

    #[test]
    fn string_literal_parser_rejects_invalid_delimiters_and_unescaped_chars() {
        assert!(StringLiteral::<Error<&str>>::new().parse("").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("member").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"unterminated").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("'unterminated").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\u{0000}\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"line\nbreak\"").is_err());
        assert!(
            StringLiteral::<Error<&str>>::new()
                .parse("\"control\u{001F}\"")
                .is_err()
        );
        assert!(StringLiteral::<Error<&str>>::new().parse("'\u{0000}'").is_err());
    }

    #[test]
    fn string_literal_parser_rejects_invalid_escape_sequences() {
        assert!(StringLiteral::<Error<&str>>::new().parse("\"bad\\q\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"bad\\0\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"bad\\x\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"dangling\\").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\'\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("'\\\"'").is_err());
    }

    #[test]
    fn string_literal_parser_rejects_invalid_unicode_escapes() {
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\u\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\u123\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"bad\\u00ZZ\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uD800\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uD800x\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uD800\\uD7FF\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uD800\\uE000\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uDBFF\\u0041\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uDC00\"").is_err());
        assert!(StringLiteral::<Error<&str>>::new().parse("\"\\uDFFF\"").is_err());
    }
}
