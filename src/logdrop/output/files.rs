use std::collections::HashMap;
use std::collections::hashmap::{Vacant, Occupied};
use std::io::{File, Append, Write};

use serialize::json::{String, List, Object};

use time;

use super::Output;

use logdrop::Payload;
use logdrop::logger::{Debug, Info, Warn};

#[deriving(Show, Clone, PartialEq)]
enum ParserError {
    EOFWhileParsingPlaceholder,
}

#[deriving(Show, Clone, PartialEq)]
enum ParserEvent {
    Literal(String),
    Placeholder(Vec<String>),
    Error(ParserError),
}

#[deriving(Show, PartialEq)]
enum ParserState {
    Undefined,           // At start or after parsing value in streaming mode.
    ParsePlaceholder,    // Just after literal.
    Broken(ParserError), // Just after any error, meaning the parser will always fail from now.
}

struct FormatParser<T> {
    reader: T,
    state: ParserState,
}

impl<T: Iterator<char>> FormatParser<T> {
    fn new(reader: T) -> FormatParser<T> {
        FormatParser {
            reader: reader,
            state: Undefined
        }
    }

    fn parse(&mut self) -> Option<ParserEvent> {
        match self.reader.next() {
            Some('{') => { self.parse_placeholder() }
            Some(ch)  => { self.parse_literal(ch) }
            None      => { None }
        }
    }

    fn parse_literal(&mut self, ch: char) -> Option<ParserEvent> {
        let mut result = String::new();
        result.push(ch);

        loop {
            match self.reader.next() {
                Some('{') => {
                    self.state = ParsePlaceholder;
                    break
                }
                Some(ch) => { result.push(ch) }
                None => { break }
            }
        }

        Some(Literal(result))
    }

    fn parse_placeholder(&mut self) -> Option<ParserEvent> {
        let mut result = String::new();

        loop {
            match self.reader.next() {
                Some('}') => {
                    self.state = Undefined;
                    let result = result.as_slice().split('/').map(|s: &str| {
                        String::from_str(s)
                    }).collect();
                    return Some(Placeholder(result));
                }
                Some(c) => { result.push(c) }
                None    => {
                    self.state = Broken(EOFWhileParsingPlaceholder);
                    return Some(Error(EOFWhileParsingPlaceholder));
                }
            }
        }
    }
}

impl<T: Iterator<char>> Iterator<ParserEvent> for FormatParser<T> {
    fn next(&mut self) -> Option<ParserEvent> {
        match self.state {
            Undefined        => self.parse(),
            ParsePlaceholder => self.parse_placeholder(),
            Broken(err)      => Some(Error(err)),
        }
    }
}

#[deriving(Show, PartialEq)]
enum TokenError<'r> {
    KeyNotFound(&'r str),
    TypeMismatch,
    SyntaxError(ParserError),
}

fn consume<'r>(event: &'r ParserEvent, payload: &Payload) -> Result<String, TokenError<'r>> {
    match *event {
        Literal(ref value) => { Ok(value.clone()) }
        Placeholder(ref placeholders) => {
            let mut current = payload;
            for key in placeholders.iter() {
                match current.find(key) {
                    Some(v) => { current = v; }
                    None    => { return Err(KeyNotFound(key.as_slice())); }
                }
            }

            match *current {
                String(ref v)  => Ok(v.clone()),
                List  (ref _v) => Err(TypeMismatch),
                Object(ref _v) => Err(TypeMismatch),
                ref v @ _      => Ok(v.to_string()),
            }
        }
        Error(err) => { Err(SyntaxError(err)) }
    }
}

/// File output will write log events to files on disk.
///
/// Path can contain placeholders. For example: test.log, {source}.log, {source/host}.log
/// It creates directories and files (with append mode) automatically.
/// Log format: {timestamp} {message} by default. Can contain any attributes.
/// If attribute not found - drop event and warn.
pub struct FileOutput {
    path: Vec<ParserEvent>,
    message: Vec<ParserEvent>,
    files: HashMap<Path, File>,
}

impl FileOutput {
    pub fn new(path: &str, format: &str) -> FileOutput {
        FileOutput {
            path: FormatParser::new(path.chars()).collect(),
            message: FormatParser::new(format.chars()).collect(),
            files: HashMap::new(),
        }
    }
}

impl Output for FileOutput {
    fn feed(&mut self, payload: &Payload) {
        let mut path = String::new();
        for token in self.path.iter() {
            match consume(token, payload) {
                Ok(token) => path.push_str(token.as_slice()),
                Err(err) => {
                    log!(Warn, "Output::File" -> "dropping {} while parsing path format - {}", payload, err);
                    return;
                }
            }
        }

        let path = Path::new(path);
        let file = match self.files.entry(path.clone()) {
            Vacant(entry) => {
                log!(Info, "Output::File" -> "opening file '{}' for writing in append mode", path.display());
                entry.set(File::open_mode(&path, Append, Write).unwrap())
            }
            Occupied(entry) => entry.into_mut(),
        };

        let mut message = String::new();
        for token in self.message.iter() {
            let token = match consume(token, payload) {
                Ok(token) => token,
                Err(err) => {
                    log!(Warn, "Output::File" -> "dropping {} while parsing message format - {}", payload, err);
                    return;
                }
            };
            message.push_str(token.as_slice());
        }
        message.push('\n');

        match file.write(message.as_bytes()) {
            Ok(())   => log!(Debug, "Output::File" -> "{} bytes written", message.len()),
            Err(err) => log!(Warn, "Output::File" -> "writing error - {}", err)
        }
    }
}

#[cfg(test)]
mod test {
    extern crate test;

    use std::collections::TreeMap;

    use serialize::json::{Null, Boolean, U64, I64, F64, String, List, Object};

    use super::{FormatParser, Literal, Placeholder, Error, EOFWhileParsingPlaceholder};
    use super::{TypeMismatch, KeyNotFound};
    use super::consume;

    #[test]
    fn parse_empty_path() {
        let mut parser = FormatParser::new("".chars());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn parse_literal() {
        let mut parser = FormatParser::new("file.log".chars());
        assert_eq!(Some(Literal("file.log".to_string())), parser.next());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn parse_placeholder() {
        let mut parser = FormatParser::new("{id}".chars());
        assert_eq!(Some(Placeholder(vec!["id".to_string()])), parser.next());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn parse_placeholder_nested() {
        let mut parser = FormatParser::new("{id/source}".chars());
        assert_eq!(Some(Placeholder(vec!["id".to_string(), "source".to_string()])), parser.next());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn parse_literal_placeholder() {
        let mut parser = FormatParser::new("/directory/file.{log}".chars());
        assert_eq!(Some(Literal("/directory/file.".to_string())), parser.next());
        assert_eq!(Some(Placeholder(vec!["log".to_string()])), parser.next());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn parse_placeholder_literal() {
        let mut parser = FormatParser::new("{directory}/file.log".chars());
        assert_eq!(Some(Placeholder(vec!["directory".to_string()])), parser.next());
        assert_eq!(Some(Literal("/file.log".to_string())), parser.next());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn parse_literal_placeholder_literal() {
        let mut parser = FormatParser::new("/directory/{path}.log".chars());
        assert_eq!(Some(Literal("/directory/".to_string())), parser.next());
        assert_eq!(Some(Placeholder(vec!["path".to_string()])), parser.next());
        assert_eq!(Some(Literal(".log".to_string())), parser.next());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn break_parser_on_eof_while_parsing_placeholder() {
        let mut parser = FormatParser::new("/directory/{path".chars());
        assert_eq!(Some(Literal("/directory/".to_string())), parser.next());
        assert_eq!(Some(Error(EOFWhileParsingPlaceholder)), parser.next());
        assert_eq!(Some(Error(EOFWhileParsingPlaceholder)), parser.next());
    }

    #[test]
    fn literal_token() {
        let payload = Object(TreeMap::new());
        let token = Literal("/directory".to_string());
        assert_eq!("/directory".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_null() {
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), Null);

        let payload = Object(o);
        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!("null".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_bool() {
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), Boolean(true));
        o.insert("k2".to_string(), Boolean(false));

        let payload = Object(o);

        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!("true".to_string(), consume(&token, &payload).unwrap());

        let token = Placeholder(
            vec!["k2".to_string()],
        );
        assert_eq!("false".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_uint() {
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), U64(42u64));

        let payload = Object(o);

        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!("42".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_int() {
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), I64(-42i64));

        let payload = Object(o);

        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!("-42".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_float() {
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), F64(3.1415f64));

        let payload = Object(o);

        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!("3.1415".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_string() {
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), String("v1".to_string()));

        let payload = Object(o);
        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!("v1".to_string(), consume(&token, &payload).unwrap());
    }

    #[test]
    fn placeholder_token_fails_on_array_key() {
        let d = Vec::new();
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), List(d));

        let payload = Object(o);
        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!(Err(TypeMismatch), consume(&token, &payload));
    }

    #[test]
    fn placeholder_token_fails_on_object_key() {
        let d = TreeMap::new();
        let mut o = TreeMap::new();
        o.insert("k1".to_string(), Object(d));

        let payload = Object(o);
        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!(Err(TypeMismatch), consume(&token, &payload));
    }

    #[test]
    fn placeholder_token_fails_on_absent_key() {
        let o = TreeMap::new();

        let payload = Object(o);
        let token = Placeholder(
            vec!["k1".to_string()],
        );
        assert_eq!(Err(KeyNotFound("k1")), consume(&token, &payload));
    }

// TODO: fn placeholder_token_nested() {
}
