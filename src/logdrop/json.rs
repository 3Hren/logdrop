use std::char;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    F64(f64),
    String(String),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn find(&self, key: &str) -> Option<&Value> {
        match self {
            &Value::Object(ref map) => map.get(key),
            _ => None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    ExpectedValue,                      // Expected any valid value.
    ExpectedValueOrArrayEnd,            // Expected value or closing ']' character.
    ExpectedKeyOrObjectEnd,             // Expected object key as string or closing '}' character.
    ExpectedColon,                      // Expected ':' character after object key, but found the other one.
    EOFWhileParsingString,              // Unexpected EOF while parsing string.
    EOFWhileParsingArray,               // Unexpected EOF while parsing array.
    EOFWhileParsingObject,              // Unexpected EOF while parsing object.
    EOFWhileParsingObjectKey,           // Unexpected EOF while parsing object key.
    EOFWhileParsingObjectColon,         // Unexpected EOF while parsing object colon.
    EOFWhileParsingObjectValue,         // Unexpected EOF while parsing object value.
    InvalidEscape,                      // Invalid escaped characters while parsing string.
    InvalidUnicodeCodePoint,
    LoneLeadingSurrogateInHexEscape,
    UnexpectedEndOfHexEscape,
    ToDo,
}

//impl Debug for Error {
//    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
//        let reason = match *self {
//            Error::ExpectedValue              => "invalid value - expected `null`, `true`, `false`, `number`, `string`, `[` or `{`",
//            Error::ExpectedValueOrArrayEnd    => "invalid array - expected `null`, `true`, `false`, `number`, `string`, `{`, `[` or `]`",
//            Error::ExpectedKeyOrObjectEnd     => "invalid object - expected `string` or `}`",
//            Error::ExpectedColon              => "invalid object - expected `:` after object key",
//            Error::EOFWhileParsingString      => "unexpected EOF while parsing string",
//            Error::EOFWhileParsingArray       => "unexpected EOF while parsing array",
//            Error::EOFWhileParsingObject      => "unexpected EOF while parsing object",
//            Error::EOFWhileParsingObjectKey   => "unexpected EOF while parsing object key",
//            Error::EOFWhileParsingObjectColon => "unexpected EOF while parsing object colon",
//            Error::EOFWhileParsingObjectValue => "unexpected EOF while parsing object value",
//            Error::InvalidEscape              => "invalid escaped characters while parsing string",
//            Error::InvalidUnicodeCodePoint    => "invalid unicode code point",
//            Error::LoneLeadingSurrogateInHexEscape => "lone leading surrogate in hex escape",
//            Error::UnexpectedEndOfHexEscape   => "unexpected end of hex escape",
//            Error::ToDo                       => "todo"
//        };
//        reason.fmt(f)
//    }
//}

#[derive(Debug, Clone, PartialEq)]
pub enum ParserError {
    SyntaxError(Error), // TODO: Rename to InvalidSyntax
    BrokenParser,
    IOError // TODO:Rename to Io(io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonEvent { // TODO: Rename to Event.
    NullValue,
    BooleanValue(bool),
    NumberValue(f64),
    StringValue(String),
    ArrayBegin,
    ArrayEnd,
    ObjectBegin,
    ObjectEnd,
    Error(ParserError)
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum ParserState {
    Undefined,          // At start or after parsing value in streaming mode.
    Broken,             // Just after any error, meaning the parser always fails from now.
    ParseArray,         // Just after array begin.
    ParseArrayMaybe,    // Just after array element.
    ParseObject,        // Just after object begin.
    ParseObjectPair,    // Just after object key.
    ParseObjectMaybe,   // Just after object value.
}

pub struct Parser<T> {
    reader: T,
    ch: Option<char>,
    handled: bool,
    state: ParserState,
    stack: Vec<ParserState>,
}

impl<T: Iterator<Item = char>> Parser<T> {
    pub fn new(reader: T) -> Parser<T> {
        Parser {
            reader: reader,
            ch: Some('\x00'),
            handled: true,
            state: ParserState::Undefined,
            stack: Vec::new()
        }
    }

    fn parse(&mut self) -> Option<JsonEvent> {
        match self.state {
            ParserState::Undefined => {
                if self.eof() {
                    None
                } else {
                    Some(self.parse_value())
                }
            }
            ParserState::Broken           => { Some(JsonEvent::Error(ParserError::BrokenParser)) }
            ParserState::ParseArray       => { Some(self.parse_array(true)) }
            ParserState::ParseArrayMaybe  => { Some(self.parse_array(false)) }
            ParserState::ParseObject      => { Some(self.parse_object(true)) }
            ParserState::ParseObjectPair  => { Some(self.parse_object_value()) }
            ParserState::ParseObjectMaybe => { Some(self.parse_object(false)) }
        }
    }

    fn parse_value(&mut self) -> JsonEvent {
        match self.char() {
            'n' => self.complete("ull", JsonEvent::NullValue),
            't' => self.complete("rue", JsonEvent::BooleanValue(true)),
            'f' => self.complete("alse", JsonEvent::BooleanValue(false)),
            '-' | '0'...'9'  => self.parse_number(),
            '"' => {
                self.bump();
                self.parse_string()
            }
            '[' => {
                self.stack.push(self.state);
                self.state = ParserState::ParseArray;
                self.handled = true;
                JsonEvent::ArrayBegin
            }
            '{' => {
                self.stack.push(self.state);
                self.state = ParserState::ParseObject;
                self.handled = true;
                JsonEvent::ObjectBegin
            }
            _   => {
                self.syntax_error(Error::ExpectedValue)
            }
        }
    }

    fn syntax_error(&mut self, error: Error) -> JsonEvent {
        self.state = ParserState::Broken;
        JsonEvent::Error(ParserError::SyntaxError(error))
    }

    fn parse_array(&mut self, first: bool) -> JsonEvent {
        self.whitespaces();

        if self.eof() {
            return self.syntax_error(Error::EOFWhileParsingArray);
        }

        match self.char() {
            ']' => {
                self.state = self.stack.pop().unwrap();
                self.handled = true;
                JsonEvent::ArrayEnd
            }
            ',' => {
                self.bump();
                if first {
                    self.syntax_error(Error::ExpectedValueOrArrayEnd)
                } else {
                    self.parse_array(false)
                }
            }
            _ => {
                self.state = ParserState::ParseArrayMaybe;
                self.parse_value()
            }
        }
    }

    fn parse_object(&mut self, first: bool) -> JsonEvent {
        self.whitespaces();
        if self.eof() {
            return self.syntax_error(Error::EOFWhileParsingObject);
        }

        match self.char() {
            '}' => {
                self.state = self.stack.pop().unwrap();
                self.handled = true;
                JsonEvent::ObjectEnd
            }
            '"' => {
                self.state = ParserState::ParseObjectPair;
                self.bump();
                self.parse_string()
            }
            ',' => {
                self.bump();
                if first {
                    self.syntax_error(Error::ExpectedKeyOrObjectEnd)
                } else {
                    self.parse_object(false)
                }
            }
            _ => {
                self.syntax_error(Error::ExpectedKeyOrObjectEnd)
            }
        }
    }

    fn parse_object_value(&mut self) -> JsonEvent {
        self.whitespaces();
        if self.eof() {
            return self.syntax_error(Error::EOFWhileParsingObjectColon);
        }

        if self.char() != ':' {
            return self.syntax_error(Error::ExpectedColon);
        }

        self.bump();
        self.whitespaces();
        if self.eof() {
            return self.syntax_error(Error::EOFWhileParsingObjectValue);
        }

        self.state = ParserState::ParseObjectMaybe;
        self.parse_value()
    }

    fn parse_number(&mut self) -> JsonEvent {
        match self.parse_number_impl() {
            Ok(result) => { JsonEvent::NumberValue(result) }
            Err(error) => {
                self.state = ParserState::Broken;
                JsonEvent::Error(error)
            }
        }
    }

    fn parse_number_impl(&mut self) -> Result<f64, ParserError> {
        let negative = if self.char() == '-' {
            self.bump();
            true
        } else {
            false
        };

        // Parse integer values until EOF or non-integer value found.
        let mut integer = 0;
        match self.char() {
            '0' => {
                self.bump();
                match self.char() {
                    // A leading '0' must be the only digit before the decimal point or other non-integer symbol.
                    '0'...'9' => { return Err(ParserError::SyntaxError(Error::ToDo)) }
                    _        => {}
                }
            }
            '1'...'9' => {
                while !self.eof() {
                    match self.char() {
                        c @ '0'...'9' => {
                            integer *= 10;
                            integer += ((c as isize) - ('0' as isize)) as u64;
                        }
                        _ => break,
                    }

                    self.bump();
                }
            }
            _ => {
                // !
                return Err(ParserError::SyntaxError(Error::ToDo))
            }
        };

        // Parse decimal.
        let mut decimal = 0.0;
        if self.char() == '.' {
            self.bump();
            match self.char() {
                '0'...'9' => (),
                // !
                 _ => return Err(ParserError::SyntaxError(Error::ToDo))
            }

            let mut dec = 1.0;
            while !self.eof() {
                match self.char() {
                    c @ '0'...'9' => {
                        dec /= 10.0;
                        decimal += (((c as isize) - ('0' as isize)) as f64) * dec;
                    }
                    _ => break,
                }

                self.bump();
            }
        }

        let mantissa = integer as f64 + decimal;

        // Parse exponent.
        let mut exponent = 0;
//        let mut negative_exponent = false;

        match self.char() {
            'e' | 'E' => {
                self.bump();

                if self.char() == '+' {
                    self.bump();
                } else if self.char() == '-' {
//                    negative_exponent = true;
                    self.bump();
                }

                // Make sure a digit follows the exponent place.
                match self.char() {
                    '0'...'9' => (),
                        // !
                    _ => return Err(ParserError::SyntaxError(Error::ToDo))
                }

                while !self.eof() {
                    match self.char() {
                        c @ '0'...'9' => {
                            exponent *= 10;
                            exponent += (c as usize) - ('0' as usize);
                        }
                        _ => break
                    }

                    self.bump();
                }
            }
            _ => {}
        }

        let result = mantissa * 10f64.powi(exponent as i32);
        self.handled = false;

        if self.eof() {
            match self.state {
                ParserState::ParseArrayMaybe  => { return Err(ParserError::SyntaxError(Error::EOFWhileParsingArray)) }
                ParserState::ParseObjectMaybe => { return Err(ParserError::SyntaxError(Error::EOFWhileParsingObjectValue)) }
                _                => {}
            }
        }

        return Ok(match negative {
            true  => -result,
            false => result
        });
    }

    fn parse_string(&mut self) -> JsonEvent {
        match self.parse_string_impl() {
            Ok(string) => JsonEvent::StringValue(string),
            Err(error) => {
                self.state = ParserState::Broken;
                JsonEvent::Error(error)
            }
        }
    }

    fn parse_string_impl(&mut self) -> Result<String, ParserError> {
        let mut result = String::new();
        let mut escape = false;

        loop {
            if self.eof() {
                return match self.state {
                    ParserState::ParseObjectPair => {
                        Err(ParserError::SyntaxError(Error::EOFWhileParsingObjectKey))
                    }
                    _ => Err(ParserError::SyntaxError(Error::EOFWhileParsingString))
                }
            }

            if escape {
                match self.char() {
                    '"'  => result.push('"'),
                    '\\' => result.push('\\'),
                    '/'  => result.push('/'),
                    'b'  => result.push('\x08'),
                    'f'  => result.push('\x0c'),
                    'n'  => result.push('\n'),
                    'r'  => result.push('\r'),
                    't'  => result.push('\t'),
                    'u' => match try!(self.decode_hex_escape()) {
                        0xDC00 ... 0xDFFF => return Err(ParserError::SyntaxError(Error::LoneLeadingSurrogateInHexEscape)),

                        // Non-BMP characters are encoded as a sequence of
                        // two hex escapes, representing UTF-16 surrogates.
//                        n1 @ 0xD800 ... 0xDBFF => {
//                            match (self.next_char(), self.next_char()) {
//                                (Some('\\'), Some('u')) => (),
//                                _ => return Err(ParserError::SyntaxError(Error::UnexpectedEndOfHexEscape)),
//                            }

//                            let buf = [n1, try!(self.decode_hex_escape())];
//                            match str::utf16_items(buf.as_slice()).next() {
//                                Some(ScalarValue(c)) => result.push(c),
//                                _ => return Err(ParserError::SyntaxError(Error::LoneLeadingSurrogateInHexEscape)),
//                            }
//                        }

                        n => match char::from_u32(n as u32) {
                            Some(c) => result.push(c),
                            None => return Err(ParserError::SyntaxError(Error::InvalidUnicodeCodePoint)),
                        },
                    },
                    _    => { return Err(ParserError::SyntaxError(Error::InvalidEscape)) }
                }
                escape = false;
            } else if self.char() == '\\' {
                escape = true;
            } else {
                match self.char() {
                    '"' => {
                        self.handled = true;
                        return Ok(result);
                    },
                    c => result.push(c)
                }
            }

            self.bump();
        }
    }

    fn complete(&mut self, ident: &str, value: JsonEvent) -> JsonEvent {
        if ident.chars().all(|c| Some(c) == self.next_char()) {
            self.handled = true;
            value
        } else {
            self.syntax_error(Error::ExpectedValue)
        }
    }

    fn whitespaces(&mut self) {
        loop {
            match self.char() {
                ' ' | '\n' | '\t' | '\r' => { self.bump() }
                _ => break
            }
        }
    }

    fn bump(&mut self) {
        self.ch = self.reader.next();
    }

    fn eof(&mut self) -> bool {
        return self.ch.is_none()
    }

    fn char(&mut self) -> char {
        return self.ch.unwrap_or('\x00');
    }

    fn next_char(&mut self) -> Option<char> {
        self.bump();
        return Some(self.char());
    }

    fn decode_hex_escape(&mut self) -> Result<u16, ParserError> {
        let mut i = 0;
        let mut n = 0u16;
        while i < 4 && !self.eof() {
            self.bump();
            n = match self.char() {
                c @ '0' ... '9' => n * 16 + ((c as u16) - ('0' as u16)),
                'a' | 'A' => n * 16 + 10,
                'b' | 'B' => n * 16 + 11,
                'c' | 'C' => n * 16 + 12,
                'd' | 'D' => n * 16 + 13,
                'e' | 'E' => n * 16 + 14,
                'f' | 'F' => n * 16 + 15,
                _ => return Err(ParserError::SyntaxError(Error::InvalidEscape))
            };

            i += 1;
        }

        // Error out if we didn't parse 4 digits.
        if i != 4 {
            return Err(ParserError::SyntaxError(Error::InvalidEscape));
        }

        Ok(n)
    }
}

impl<T: Iterator<Item = char>> Iterator for Parser<T> {
    type Item = JsonEvent;

    fn next(&mut self) -> Option<JsonEvent> {
        if self.state == ParserState::Broken {
            return Some(JsonEvent::Error(ParserError::BrokenParser));
        }

        if self.handled {
            self.handled = false;
            self.bump();
        }

        self.parse()
    }
}

pub struct Builder<T> {
    parser: Parser<T>,
    arrays: Vec<bool>
}

impl<T: Iterator<Item = char>> Builder<T> {
    pub fn new(src: T) -> Builder<T> {
        Builder {
            parser: Parser::new(src),
            arrays: Vec::new()
        }
    }
}

impl<T: Iterator<Item = char>> Iterator for Builder<T> {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        match self.parser.next() {
            Some(JsonEvent::NullValue) => Some(Value::Null),
            Some(JsonEvent::BooleanValue(v)) => Some(Value::Bool(v)),
            Some(JsonEvent::NumberValue(v)) => Some(Value::F64(v)),
            Some(JsonEvent::StringValue(v)) => Some(Value::String(v)),
            Some(JsonEvent::ArrayBegin) => {
                let mut array = Vec::new();
                self.arrays.push(false);
                loop {
                    let element = match self.next() {
                        Some(v) => v,
                        None => {
                            if *self.arrays.last().unwrap() {
                                self.arrays.pop();
                                return Some(Value::List(array));
                            } else {
                                return None;
                            }
                        }
                    };
                    array.push(element);
                }
            }
            Some(JsonEvent::ObjectBegin) => {
                let mut object = BTreeMap::new();
                loop {
                    let key = match self.parser.next().unwrap() {
                        JsonEvent::StringValue(v) => v,
                        JsonEvent::ObjectEnd => return Some(Value::Object(object)),
                        _ => panic!("parse error - must be key or object end")
                    };
                    let value = self.next().unwrap();
                    object.insert(key, value);
                }
            }
            Some(JsonEvent::ArrayEnd) => {
                *self.arrays.last_mut().unwrap() = true;
                return None;
            }
            Some(JsonEvent::ObjectEnd) => unreachable!(),
            Some(JsonEvent::Error(err)) => panic!(err),
            None => None
        }
    }
}

#[cfg(test)]
mod testing {

use super::*;

#[test]
fn parse_null() {
    let mut parser = Parser::new("null".chars());

    assert_eq!(Some(JsonEvent::NullValue), parser.next());
    assert_eq!(None, parser.next());
}

//#[test]
//fn parse_true() {
//    let mut parser = Parser::new("true".chars());
//    assert_eq!(Some(BooleanValue(true)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_false() {
//    let mut parser = Parser::new("false".chars());
//    assert_eq!(Some(BooleanValue(false)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_int_null() {
//    let mut parser = Parser::new("0".chars());
//    assert_eq!(Some(NumberValue(0.0)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_int_value() {
//    let mut parser = Parser::new("42".chars());
//    assert_eq!(Some(NumberValue(42.0)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_int_negative_value() {
//    let mut parser = Parser::new("-42".chars());
//    assert_eq!(Some(NumberValue(-42.0)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_float_null() {
//    let mut parser = Parser::new("0.0".chars());
//    assert_eq!(Some(NumberValue(0.0)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_float_value() {
//    let mut parser = Parser::new("42.5".chars());
//    assert_eq!(Some(NumberValue(42.5)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_float_negative_value() {
//    let mut parser = Parser::new("-42.5".chars());
//    assert_eq!(Some(NumberValue(-42.5)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_float_e_value() {
//    let mut parser = Parser::new("42e2".chars());
//    assert_eq!(Some(NumberValue(42e2)), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_string() {
//    let mut parser = Parser::new(r#""value""#.chars());
//    assert_eq!(Some(StringValue("value".to_string())), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_empty_array() {
//    let mut parser = Parser::new("[]".chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(ArrayEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_array_with_single_int() {
//    let mut parser = Parser::new("[42]".chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(NumberValue(42.0)), parser.next());
//    assert_eq!(Some(ArrayEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_array_with_multiple_ints() {
//    let mut parser = Parser::new("[42,43]".chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(NumberValue(42.0)), parser.next());
//    assert_eq!(Some(NumberValue(43.0)), parser.next());
//    assert_eq!(Some(ArrayEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_array_with_variant() {
//    let mut parser = Parser::new(r#"[null, true, false, 42.5, "string", [], {}]"#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(NullValue), parser.next());
//    assert_eq!(Some(BooleanValue(true)), parser.next());
//    assert_eq!(Some(BooleanValue(false)), parser.next());
//    assert_eq!(Some(NumberValue(42.5)), parser.next());
//    assert_eq!(Some(StringValue("string".to_string())), parser.next());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(ArrayEnd), parser.next());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(Some(ArrayEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_empty_object() {
//    let mut parser = Parser::new("{}".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_object_kv() {
//    let mut parser = Parser::new(r#"{"key":"value"}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(StringValue("value".to_string())), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_objects_nested() {
//    let mut parser = Parser::new(r#"{"outer":{"inner":"value"}}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("outer".to_string())), parser.next());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("inner".to_string())), parser.next());
//    assert_eq!(Some(StringValue("value".to_string())), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_objects_multiple() {
//    let mut parser = Parser::new(r#"{"first":1,"second":2}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("first".to_string())), parser.next());
//    assert_eq!(Some(NumberValue(1.0)), parser.next());
//    assert_eq!(Some(StringValue("second".to_string())), parser.next());
//    assert_eq!(Some(NumberValue(2.0)), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_objects_multiple_inner() {
//    let mut parser = Parser::new(r#"{"k1":"v1","k2":{"k3":42},"k4":"v4"}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("k1".to_string())), parser.next());
//    assert_eq!(Some(StringValue("v1".to_string())), parser.next());
//    assert_eq!(Some(StringValue("k2".to_string())), parser.next());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("k3".to_string())), parser.next());
//    assert_eq!(Some(NumberValue(42.0)), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(Some(StringValue("k4".to_string())), parser.next());
//    assert_eq!(Some(StringValue("v4".to_string())), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//#[test]
//fn parse_multiple_values_streamed() {
//    let mut parser = Parser::new(r#"{}{}nulltruefalse42"string"42.5[true]{}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(Some(NullValue), parser.next());
//    assert_eq!(Some(BooleanValue(true)), parser.next());
//    assert_eq!(Some(BooleanValue(false)), parser.next());
//    assert_eq!(Some(NumberValue(42.0)), parser.next());
//    assert_eq!(Some(StringValue("string".to_string())), parser.next());
//    assert_eq!(Some(NumberValue(42.5)), parser.next());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(BooleanValue(true)), parser.next());
//    assert_eq!(Some(ArrayEnd), parser.next());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(ObjectEnd), parser.next());
//    assert_eq!(None, parser.next());
//}

//// Parser error test case

//#[test]
//fn parse_error_syntax_null() {
//    let mut parser = Parser::new(r#"n"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"nu"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"nul"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"nulo"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_syntax_true() {
//    let mut parser = Parser::new(r#"t"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"tr"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"tru"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"truo"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_syntax_false() {
//    let mut parser = Parser::new(r#"f"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"fa"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"fal"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"fals"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"falso"#.chars());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_string_eof() {
//    let mut parser = Parser::new("[\"".chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingString))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new("[\"le".chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingString))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_eof_while_parsing_array() {
//    let mut parser = Parser::new(r#"["#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingArray))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"[null"#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(NullValue), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingArray))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"[null,"#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(NullValue), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingArray))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"[null, [42"#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(NullValue), parser.next());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingArray))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_array_starting_with_comma() {
//    let mut parser = Parser::new(r#"[,"#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValueOrArrayEnd))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new(r#"[,null]"#.chars());
//    assert_eq!(Some(ArrayBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValueOrArrayEnd))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_eof_while_parsing_object() {
//    let mut parser = Parser::new(r#"{"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObject))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_eof_while_parsing_object_key() {
//    let mut parser = Parser::new("{\"key".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObjectKey))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_eof_while_parsing_just_after_object_key_parsed() {
//    let mut parser = Parser::new("{\"key\"".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObjectColon))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_eof_while_parsing_object_value() {
//    let mut parser = Parser::new("{\"key\":".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObjectValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new("{\"key\":4".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObjectValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new("{\"key\":42".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObjectValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());

//    parser = Parser::new("{\"key\": {\"a\": 42".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("a".to_string())), parser.next());
//    assert_eq!(Some(Error(SyntaxError(EOFWhileParsingObjectValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_expected_colon_while_parsing_object() {
//    let mut parser = Parser::new("{\"key\".".chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(StringValue("key".to_string())), parser.next());
//    assert_eq!(Some(Error(SyntaxError(ExpectedColon))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_object_starting_with_comma() {
//    let mut parser = Parser::new(r#"{,}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(ExpectedKeyOrObjectEnd))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_object_starting_not_with_string_key() {
//    let mut parser = Parser::new(r#"{null:42}"#.chars());
//    assert_eq!(Some(ObjectBegin), parser.next());
//    assert_eq!(Some(Error(SyntaxError(ExpectedKeyOrObjectEnd))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_invalid_number() {
//    let mut parser = Parser::new(r#"42l"#.chars());
//    assert_eq!(Some(NumberValue(42f64)), parser.next());
//    assert_eq!(Some(Error(SyntaxError(ExpectedValue))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

//#[test]
//fn parse_error_invalid_escape() {
//    let mut parser = Parser::new("\"escape\\l\"".chars());
//    assert_eq!(Some(Error(SyntaxError(InvalidEscape))), parser.next());
//    assert_eq!(Some(Error(BrokenParser)), parser.next());
//}

// Builder test case.

#[test]
fn build_null() {
    let mut builder = Builder::new("null".chars());
    assert_eq!(Some(Value::Null), builder.next());
    assert_eq!(None, builder.next());
}

//#[test]
//fn build_true() {
//    let mut builder = Builder::new("true".chars());
//    assert_eq!(Some(Boolean(true)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_false() {
//    let mut builder = Builder::new("false".chars());
//    assert_eq!(Some(Boolean(false)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_number() {
//    let mut builder = Builder::new("42".chars());
//    assert_eq!(Some(F64(42.0)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_string() {
//    let mut builder = Builder::new(r#""42""#.chars());
//    assert_eq!(Some(String("42".to_string())), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_empty_array() {
//    let mut builder = Builder::new("[]".chars());
//    let d = Vec::new();
//    assert_eq!(Some(List(d)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_array_single_value() {
//    let mut builder = Builder::new(r#"["item"]"#.chars());

//    let mut d = Vec::new();
//    d.push(String("item".to_string()));

//    assert_eq!(Some(List(d)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_array_multiple_values() {
//    let mut builder = Builder::new(r#"["i1","i2"]"#.chars());

//    let mut d = Vec::new();
//    d.push(String("i1".to_string()));
//    d.push(String("i2".to_string()));

//    assert_eq!(Some(List(d)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_empty_object() {
//    use serialize::json::Object;
//    let mut builder = Builder::new("{}".chars());
//    let d = TreeMap::new();
//    assert_eq!(Some(Object(d)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_kv_object() {
//    use serialize::json::Object;
//    let mut builder = Builder::new(r#"{"k1":"v1"}"#.chars());
//    let mut d = TreeMap::new();
//    d.insert("k1".to_string(), String("v1".to_string()));

//    assert_eq!(Some(Object(d)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_objects_nested() {
//    use serialize::json::Object;
//    let mut builder = Builder::new(r#"{"k1":{"k2":"v2"}}"#.chars());

//    let mut o2 = TreeMap::new();
//    o2.insert("k2".to_string(), String("v2".to_string()));

//    let mut o1 = TreeMap::new();
//    o1.insert("k1".to_string(), Object(o2));

//    assert_eq!(Some(Object(o1)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_objects_multiple() {
//    use serialize::json::Object;
//    let mut builder = Builder::new(r#"{"k1":"v1","k2":"v2"}"#.chars());

//    let mut o = TreeMap::new();
//    o.insert("k1".to_string(), String("v1".to_string()));
//    o.insert("k2".to_string(), String("v2".to_string()));

//    assert_eq!(Some(Object(o)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_objects_multiple_nested() {
//    use serialize::json::Object;
//    let mut builder = Builder::new(r#"{"k1":"v1","k2":{"k3":"v3","k4":"v4"},"k5":"v5"}"#.chars());

//    let mut o2 = TreeMap::new();
//    o2.insert("k3".to_string(), String("v3".to_string()));
//    o2.insert("k4".to_string(), String("v4".to_string()));

//    let mut o1 = TreeMap::new();
//    o1.insert("k1".to_string(), String("v1".to_string()));
//    o1.insert("k2".to_string(), Object(o2));
//    o1.insert("k5".to_string(), String("v5".to_string()));

//    assert_eq!(Some(Object(o1)), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_multiple_values_streamed() {
//    let mut builder = Builder::new(r#"{}{}nulltruefalse42"string"42.5[true]{}"#.chars());
//    assert_eq!(Some(Object(TreeMap::new())), builder.next());
//    assert_eq!(Some(Object(TreeMap::new())), builder.next());
//    assert_eq!(Some(Null), builder.next());
//    assert_eq!(Some(Boolean(true)), builder.next());
//    assert_eq!(Some(Boolean(false)), builder.next());
//    assert_eq!(Some(F64(42.0)), builder.next());
//    assert_eq!(Some(String("string".to_string())), builder.next());
//    assert_eq!(Some(F64(42.5)), builder.next());
//    assert_eq!(Some(List(Vec::from_slice([Boolean(true)]))), builder.next());
//    assert_eq!(Some(Object(TreeMap::new())), builder.next());
//    assert_eq!(None, builder.next());
//}

//#[test]
//fn build_objects_multiple_nested_arrays() {
//    use serialize::json::Object;
//    let mut builder = Builder::new(r#"["k1",{"k2":["v2"]},[42]]"#.chars());

//    let mut d = Vec::new();
//    d.push(String("k1".to_string()));

//    let mut o = TreeMap::new();
//    o.insert("k2".to_string(), List(Vec::from_slice([String("v2".to_string())])));
//    d.push(Object(o));
//    d.push(List(Vec::from_slice([F64(42.0)])));

//    assert_eq!(Some(List(d)), builder.next());
//    assert_eq!(None, builder.next());
//}

//mod parser {
//    use super::super::{StringValue};
//    use super::super::{Parser};

//    #[test]
//    fn string_escape() {
//        let raw = r#""foo\nbar""#;
//        let mut parser = Parser::new(raw.chars());
//        assert_eq!(Some(StringValue("foo\nbar".to_string())), parser.next());
//        assert_eq!(None, parser.next());
//    }
//} // mod parser

//#[test]
//fn small() {
//    let raw = r#"{
//        "a": 1.0,
//        "b": [
//            true,
//            "foo\nbar",
//            { "c": {"d": null} }
//        ]
//    }"#;

//    let mut o2 = TreeMap::new();
//    o2.insert("d".to_string(), Null);

//    let mut o1 = TreeMap::new();
//    o1.insert("c".to_string(), Object(o2));

//    let mut a1 = Vec::new();
//    a1.push(Boolean(true));
//    a1.push(String("foo\nbar".to_string()));
//    a1.push(Object(o1));

//    let mut expected = TreeMap::new();
//    expected.insert("a".to_string(), F64(1.0));
//    expected.insert("b".to_string(), List(a1));

//    let mut builder = Builder::new(raw.chars());
//    assert_eq!(Some(Object(expected)), builder.next());
//    assert_eq!(None, builder.next());
//}

} // mod test

#[cfg(test)]
mod benchmarking {

extern crate test;

use self::test::Bencher;

use super::{Builder};

//use serialize::json;
//use serialize::json::{Parser};

#[bench]
fn small(b: &mut Bencher) {
    let raw = r#"{
        "a": 1.0,
        "b": [
            true,
            "foo\nbar",
            { "c": {"d": null} }
        ]
    }"#;

    b.iter(|| {
        let mut builder = Builder::new(raw.chars());
        loop {
            match builder.next() {
                None => break,
                Some(c) => {
                    test::black_box(c);
                }
            }
        }
    });
}

//#[bench]
//fn small_std(b: &mut Bencher) {
//    let raw = r#"{
//        "a": 1.0,
//        "b": [
//            true,
//            "foo\nbar",
//            { "c": {"d": null} }
//        ]
//    }"#;
//    b.iter( || {
//        let _ = json::from_str(raw);
//    });
//}

} // mod benchmarking
