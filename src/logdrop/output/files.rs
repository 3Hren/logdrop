use std::collections::HashMap;
use std::fs::{File, OpenOptions, PathExt};
use std::io::Write;
use std::path::Path;

use libc;

use super::super::Record;
use super::Output;

#[derive(Copy, Clone, Debug, PartialEq)]
enum ParserError {
    EOFWhileParsingPlaceholder,
}

#[derive(Debug, Clone, PartialEq)]
enum ParserEvent {
    Literal(String),
    Placeholder(Vec<String>),
    Error(ParserError),
}

#[derive(Debug, PartialEq)]
enum ParserState {
    Undefined,           // At start or after parsing value in streaming mode.
    ParsePlaceholder,    // Just after literal.
    Broken(ParserError), // Just after any error, meaning the parser will always fail from now.
}

struct FormatParser<T> {
    reader: T,
    state: ParserState,
}

impl<T: Iterator<Item = char>> FormatParser<T> {
    fn new(reader: T) -> FormatParser<T> {
        FormatParser {
            reader: reader,
            state: ParserState::Undefined
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
                    self.state = ParserState::ParsePlaceholder;
                    break
                }
                Some(ch) => { result.push(ch) }
                None => { break }
            }
        }

        Some(ParserEvent::Literal(result))
    }

    fn parse_placeholder(&mut self) -> Option<ParserEvent> {
        let mut result = String::new();

        loop {
            match self.reader.next() {
                Some('}') => {
                    self.state = ParserState::Undefined;
                    let result = result.split('/').map(|v| {
                        v.to_string()
                    }).collect();
                    return Some(ParserEvent::Placeholder(result));
                }
                Some(c) => { result.push(c) }
                None    => {
                    self.state = ParserState::Broken(ParserError::EOFWhileParsingPlaceholder);
                    return Some(ParserEvent::Error(ParserError::EOFWhileParsingPlaceholder));
                }
            }
        }
    }
}

impl<T: Iterator<Item = char>> Iterator for FormatParser<T> {
    type Item = ParserEvent;

    fn next(&mut self) -> Option<ParserEvent> {
        match self.state {
            ParserState::Undefined        => self.parse(),
            ParserState::ParsePlaceholder => self.parse_placeholder(),
            ParserState::Broken(err)      => Some(ParserEvent::Error(err)),
        }
    }
}

#[derive(Debug, PartialEq)]
enum TokenError<'r> {
    KeyNotFound(&'r str),
    TypeMismatch,
    SyntaxError(ParserError),
}

fn consume<'r>(event: &'r ParserEvent, payload: &Record) -> Result<String, TokenError<'r>> {
    match *event {
        ParserEvent::Literal(ref value) => { Ok(value.clone()) }
        ParserEvent::Placeholder(ref placeholders) => {
            let mut current = payload;
            for key in placeholders.iter() {
                match current.find(key) {
                    Some(v) => { current = v; }
                    None    => { return Err(TokenError::KeyNotFound(&key)); }
                }
            }

            match *current {
                RecordItem::String(ref v) => Ok(v.clone()),
                RecordItem::Array(..) => Err(TokenError::TypeMismatch),
                RecordItem::Object(..) => Err(TokenError::TypeMismatch),
                ref other => Ok(format!("{:?}", other)),
            }
        }
        ParserEvent::Error(err) => { Err(TokenError::SyntaxError(err)) }
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
    files: HashMap<u64, File>,
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
    fn feed(&mut self, payload: &Record) {
        let mut path = String::new();
        for token in self.path.iter() {
            match consume(token, payload) {
                Ok(token) => path.push_str(&token),
                Err(err) => {
                    warn!(target: "Output::File", "dropping {:?} while parsing path format - {:?}", payload, err);
                    return;
                }
            }
        }

        let path = Path::new(&path);
        let mut stat = libc::stat {
            st_dev: 0,
            st_ino: 0,
            st_nlink: 0,
            st_mode: 0,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 0,
            st_blocks: 0,
            st_atime: 0,
            st_atime_nsec: 0,
            st_mtime: 0,
            st_mtime_nsec: 0,
            st_ctime: 0,
            st_ctime_nsec: 0,
            st_birthtime: 0,
            st_birthtime_nsec: 0,
            st_flags: 0,
            st_gen: 0,
            st_lspare: 0,
            st_qspare: [0, 2],
        };

        if !path.exists() {
            File::create(path).unwrap();
        }

        unsafe {
            if libc::stat(path.as_os_str().to_cstring().unwrap().as_ptr(), &mut stat) != 0 {
                warn!(target: "Output::File", "unable to get inode, dropping");
                return;
            }
        }

        let file = self.files.entry(stat.st_ino).or_insert_with(|| {
            info!(target: "Output::File", "opening file '{}' for writing in append mode", path.display());
            OpenOptions::new().append(true).write(true).open(&path).unwrap()
        });

        let mut message = String::new();
        for token in self.message.iter() {
            let token = match consume(token, payload) {
                Ok(token) => token,
                Err(err) => {
                    warn!(target: "Output::File", "dropping {:?} while parsing message format - {:?}", payload, err);
                    return;
                }
            };
            message.push_str(&token);
        }
        message.push('\n');

        match file.write_all(message.as_bytes()) {
            Ok(())   => debug!(target: "Output::File", "{} bytes written", message.len()),
            Err(err) => warn!(target: "Output::File", "writing error - {}", err)
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
