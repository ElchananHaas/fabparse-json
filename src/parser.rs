use fabparse::alt;
use fabparse::opt;
use fabparse::sequence::Sequence;
use fabparse::take_not;
use fabparse::FabError;
use fabparse::Parser;
use fabparse::ParserError;
use std::collections::HashMap;
use std::str::FromStr;

use crate::json::JsonValue;
/**
 * Trims all leading whitespace.
 */
fn trim_whitespace<E: ParserError>(input: &mut &str) -> Result<(), E> {
    alt(('\n', '\r', '\t', ' '))
        .fab_repeat()
        .fab_value(())
        .fab(input)
}

/**
 * There is a bit of extra complexity to handle the fact that json floats may be
 * written using an "e"
 *
 * See https://www.json.org/json-en.html
 */
fn parse_number<E: ParserError>(input: &mut &str) -> Result<f64, E> {
    let orig_input = *input;
    let _sign = opt('-').fab(input)?;
    let _before_decimal = char::is_ascii_digit.fab_repeat().min(1).fab(input)?;
    let _decimal_part = opt(('.', char::is_ascii_digit.fab_repeat().min(1))).fab(input)?;
    let _e_part = opt((
        alt(('E', 'e')),
        alt(('+', '-')),
        char::is_ascii_digit.fab_repeat().min(1),
    ))
    .fab(input)?;
    let float_str = orig_input.subtract(*input);
    let parsed: Result<f64, _> = f64::from_str(float_str);
    parsed.map_err(|err| E::from_external_error(*input, fabparse::ParserType::Function, err))
}

/**
 * Parses any char that is valid within a JSON string.
 */
fn parse_json_char<E: ParserError>(input: &mut &str) -> Result<(), E> {
    alt((
        take_not(alt((char::is_control, '\\', '"'))).fab_value(()),
        "\\\"".fab_value(()),
        "\\\\".fab_value(()),
        "\\/".fab_value(()),
        "\\b".fab_value(()),
        "\\f".fab_value(()),
        "\\n".fab_value(()),
        "\\r".fab_value(()),
        "\\t".fab_value(()),
        (
            "\\u",
            alt(('0'..='9', 'a'..='f', 'A'..='F'))
                .fab_repeat()
                .bound(4..=4),
        )
            .fab_value(()),
    ))
    .fab(input)
}
fn parse_string<E: ParserError>(input: &mut &str) -> Result<String, E> {
    ("\"", parse_json_char.fab_repeat().as_input_slice(), "\"")
        .fab_map(|(_, s, _): (&str, &str, &str)| s.to_string())
        .fab(input)
}

pub fn parse_value<E: ParserError>(input: &mut &str) -> Result<JsonValue, E> {
    trim_whitespace(input)?;
    let res = alt((
        "true".fab_value(JsonValue::Boolean(true)),
        "false".fab_value(JsonValue::Boolean(false)),
        "null".fab_value(JsonValue::Null),
        parse_string.fab_map(JsonValue::Str),
        parse_number.fab_map(JsonValue::Num),
        parse_array.fab_map(JsonValue::Array),
        parse_object.fab_map(JsonValue::Object),
    ))
    .fab(input)?;
    trim_whitespace(input)?;
    Ok(res)
}
#[derive(Clone)]
pub struct Delimited<T> {
    pub values: Vec<T>,
    //Tracks if there is a comma
    pub comma: bool,
}
fn parse_array_inner_reducer(
    acc: &mut Delimited<JsonValue>,
    val: (JsonValue, Option<char>),
) -> bool {
    if !acc.comma {
        return false;
    }
    acc.values.push(val.0);
    acc.comma = val.1.is_some();
    true
}
fn parse_array_inner<E: ParserError>(input: &mut &str) -> Result<Vec<JsonValue>, E> {
    let delim = (parse_value, opt(','))
        .fab_repeat()
        .reduce(
            Delimited {
                values: Vec::new(),
                comma: true,
            },
            parse_array_inner_reducer,
        )
        .fab(input)?;
    //JSON forbids trailing commas
    if delim.comma {
        Err(E::from_parser_error(*input, fabparse::ParserType::Function))
    } else {
        Ok(delim.values)
    }
}
fn parse_array<E: ParserError>(input: &mut &str) -> Result<Vec<JsonValue>, E> {
    '['.fab(input)?;
    let body = alt((parse_array_inner, trim_whitespace.fab_value(Vec::new()))).fab(input)?;
    ']'.fab(input)?;
    Ok(body)
}

#[derive(Clone)]
pub struct DelimitedMap {
    pub values: HashMap<String, JsonValue>,
    //Tracks if there is a comma
    pub comma: bool,
}
fn parse_object_inner_reducer(
    acc: &mut DelimitedMap,
    val: (String, JsonValue, Option<char>),
) -> bool {
    if !acc.comma {
        return false;
    }
    acc.values.insert(val.0, val.1);
    acc.comma = val.2.is_some();
    true
}
fn parse_object_item<E: ParserError>(
    input: &mut &str,
) -> Result<(String, JsonValue, Option<char>), E> {
    let _ = trim_whitespace.fab(input)?;
    let key = parse_string.fab(input)?;
    let _ = (trim_whitespace, ':').fab(input)?;
    let value = parse_value.fab(input)?;
    let comma = opt(',').fab(input)?;
    Ok((key, value, comma))
}
fn parse_object_inner<E: ParserError>(input: &mut &str) -> Result<HashMap<String, JsonValue>, E> {
    let delim = parse_object_item
        .fab_repeat()
        .reduce(
            DelimitedMap {
                values: HashMap::new(),
                comma: true,
            },
            parse_object_inner_reducer,
        )
        .fab(input)?;
    //JSON may not have trailing commas
    if delim.comma {
        Err(E::from_parser_error(*input, fabparse::ParserType::Function))
    } else {
        Ok(delim.values)
    }
}
fn parse_object<E: ParserError>(input: &mut &str) -> Result<HashMap<String, JsonValue>, E> {
    '{'.fab(input)?;
    let body = alt((parse_object_inner, trim_whitespace.fab_value(HashMap::new()),
)).fab(input)?;
    '}'.fab(input)?;
    Ok(body)
}
mod test {
    use fabparse::FabError;

    use crate::{parser::{parse_value}, json::JsonValue};

    use super::parse_number;

    #[test]
    pub fn parse_number_test() {
        let mut input = "12345";
        let out: Result<_, FabError> = parse_number(&mut input);
        assert_eq!(12345.0, out.unwrap());
    }

    #[test]
    pub fn parse_number_test_with_e() {
        let mut input = "12345.0e+2";
        let out: Result<_, FabError> = parse_number(&mut input);
        assert_eq!(1234500.0, out.unwrap());
    }
    #[test]
    pub fn parse_value_string() {
        let mut input = "\"Hello World!\"";
        let out: Result<_, FabError> = parse_value(&mut input);
        assert_eq!(JsonValue::Str("Hello World!".to_string()),out.unwrap());
    }
}
