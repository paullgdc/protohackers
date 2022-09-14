use core::fmt;
use std::{borrow::Cow, collections::HashMap, io::Write, str};

#[derive(Debug)]
pub struct Error {
    msg: &'static str,
    pos: usize,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("msg", &self.msg)
            .field("pos", &self.pos)
            .finish()
    }
}

#[derive(Debug)]
struct Cursor {
    pos: usize,
}

impl Cursor {
    fn current(&self, buf: &[u8]) -> Option<u8> {
        buf.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn next_token(&mut self, buf: &[u8]) -> u8 {
        loop {
            if self.pos >= buf.len() {
                break;
            }
            match self.current(buf).unwrap() {
                b'{'
                | b'}'
                | b'['
                | b']'
                | b'"'
                | b':'
                | b','
                | b'0'..=b'9'
                | b't'
                | b'f'
                | b'n' => return self.current(buf).unwrap(),
                b' ' | b'\n' | b'\t' | b'\r' => self.pos += 1,
                _ => return 0,
            }
        }
        0
    }

    fn consume_str<'a>(&mut self, buf: &'a [u8]) -> Result<(&'a [u8], bool), Error> {
        if self.current(buf) != Some(b'"') {
            return Err(Error {
                pos: self.pos,
                msg: "Couldn't parse string. Missed first \"",
            });
        }
        self.pos += 1;
        let mut escape = false;
        let mut escaped = false;
        let span_start = self.pos;
        loop {
            match self.current(buf) {
                Some(b'\\') if !escape => {
                    escape = true;
                    escaped = true
                }
                Some(b'"') if !escape => break,
                None => {
                    return Err(Error {
                        pos: self.pos,
                        msg: "Unexpected data end while parsing string",
                    })
                }
                _ => escape = false,
            };

            self.advance();
        }
        let span_end = self.pos;

        self.advance();
        Ok((&buf[span_start..span_end], escaped))
    }

    fn consume_number<'a>(&mut self, buf: &'a [u8]) -> Result<(&'a [u8], bool), Error> {
        let span_start = self.pos;
        let mut float = false;
        if !(b'0'..=b'9').contains(&self.current(buf).unwrap_or(0)) {
            return Err(Error {
                pos: self.pos,
                msg: "Couldn't parse number. Missed first digit",
            });
        }
        loop {
            match self.current(buf) {
                None => break,
                Some(t) => match t {
                    b'0'..=b'9' => {}
                    b'.' => {
                        float = true;
                    }
                    _ => break,
                },
            }
            self.advance();
        }
        let span_end = self.pos;
        Ok((&buf[span_start..span_end], float))
    }

    fn consume_lit(&mut self, buf: &[u8], lit: &[u8]) -> Result<(), Error> {
        if self.pos + lit.len() > buf.len() {
            return Err(Error {
                pos: self.pos,
                msg: "Unexpected data end while parsing litteral",
            });
        }
        if &buf[self.pos..self.pos + lit.len()] != lit {
            return Err(Error {
                pos: self.pos,
                msg: "Unexpected value for litteral",
            });
        }
        self.pos += lit.len();

        Ok(())
    }

    fn consume_null(&mut self, buf: &[u8]) -> Result<(), Error> {
        let null = b"null";
        self.consume_lit(buf, null)
    }

    fn consume_true(&mut self, buf: &[u8]) -> Result<(), Error> {
        let _true = b"true";
        self.consume_lit(buf, _true)
    }

    fn consume_false(&mut self, buf: &[u8]) -> Result<(), Error> {
        let _false = b"false";
        self.consume_lit(buf, _false)
    }
}

macro_rules! accessors {
    ([
        $( ( $name:ident, $variant:tt, $type:ty ) ,)*
    ]) => {
        impl <'a> Value<'a> {
            $(
                pub fn $name (&self) -> Option< $type > {
                    use Value::*;
                    match self {
                        $variant (v) => Some(v),
                        _ => None,
                    }
                }
            )*
        }
    };
}

accessors!([
    (string, String, &Cow<'_, str>),
    (int, Int, &i64),
    (float, Float, &f64),
    (bool, Bool, &bool),
    (null, Null, &()),
    (array, Array, &Vec<Value<'_>>),
    (object, Object, &HashMap<Cow<'a, str>, Value<'a>>),
]);

#[derive(Debug, PartialEq)]
pub enum Value<'a> {
    String(Cow<'a, str>),
    Float(f64),
    Int(i64),
    Bool(bool),
    Null(()),
    Array(Vec<Value<'a>>),
    Object(HashMap<Cow<'a, str>, Value<'a>>),
}

pub fn parse_json(buf: &[u8]) -> Result<Value<'_>, Error> {
    let mut cursor = Cursor { pos: 0 };
    _parse_json(buf, &mut cursor)
}

fn _parse_json<'a>(buf: &'a [u8], cursor: &mut Cursor) -> Result<Value<'a>, Error> {
    Ok(match cursor.next_token(buf) {
        b'"' => Value::String(parse_str(buf, cursor)?),
        b'0'..=b'9' => parse_number(buf, cursor)?,
        b'n' => Value::Null(parse_null(buf, cursor)?),
        b't' => Value::Bool(parse_true(buf, cursor)?),
        b'f' => Value::Bool(parse_false(buf, cursor)?),
        b'{' => Value::Object(parse_object(buf, cursor)?),
        b'[' => Value::Array(parse_array(buf, cursor)?),
        0 => {
            return Err(Error {
                pos: cursor.pos,
                msg: "Unexpected message end",
            })
        }
        _ => {
            return Err(Error {
                pos: cursor.pos,
                msg: "Unexpected token while parsing message",
            })
        }
    })
}

fn parse_str<'a, 'b>(buf: &'a [u8], cursor: &'b mut Cursor) -> Result<Cow<'a, str>, Error> {
    let (s, escaped) = cursor.consume_str(buf)?;
    Ok(if escaped {
        let mut next_char_escaped = false;
        let mut pos = 0;
        let mut unescaped = Vec::new();

        while pos < s.len() {
            let c = s[pos];
            pos += 1;

            if !next_char_escaped {
                match c {
                    b'\\' => {
                        next_char_escaped = true;
                    }
                    _ => unescaped.push(c),
                }
                continue;
            }
            next_char_escaped = false;
            match c {
                b'"' | b'\\' | b'/' => unescaped.push(c),
                b'b' => unescaped.push(0x08),  // backspace
                b'f' => unescaped.push(0xC),   // formfeed
                b'n' => unescaped.push(b'\n'), // linefeed
                b'r' => unescaped.push(b'\r'), // carriage return
                b't' => unescaped.push(b'\t'), // tab
                b'u' => {
                    return Err(Error {
                        pos: cursor.pos,
                        msg: "Hex character not handled",
                    })
                } // hex digit
                _ => {
                    return Err(Error {
                        pos: cursor.pos,
                        msg: "Unrecognised escape sequence",
                    })
                }
            }
        }
        Cow::Owned(String::from_utf8(unescaped).map_err(|_| Error {
            pos: cursor.pos,
            msg: "String wasn't utf8 encoded",
        })?)
    } else {
        Cow::Borrowed(str::from_utf8(s).map_err(|_| Error {
            pos: cursor.pos,
            msg: "String wasn't utf8 encoded",
        })?)
    })
}

fn parse_number<'a, 'b>(buf: &'a [u8], cursor: &'b mut Cursor) -> Result<Value<'a>, Error> {
    let (s, float) = cursor.consume_number(buf)?;
    let num_str = str::from_utf8(s).map_err(|_| Error {
        pos: cursor.pos,
        msg: "Couldn't decode number",
    })?;
    Ok(if float {
        Value::Float(num_str.parse().map_err(|_| Error {
            pos: cursor.pos,
            msg: "Wasn't able to parse number as float",
        })?)
    } else {
        Value::Int(num_str.parse().map_err(|_| Error {
            pos: cursor.pos,
            msg: "Wasn't able to parse number as integer",
        })?)
    })
}

fn parse_null<'a, 'b>(buf: &'a [u8], cursor: &'b mut Cursor) -> Result<(), Error> {
    cursor.consume_null(buf)
}

fn parse_true<'a, 'b>(buf: &'a [u8], cursor: &'b mut Cursor) -> Result<bool, Error> {
    cursor.consume_true(buf)?;
    Ok(true)
}

fn parse_false<'a, 'b>(buf: &'a [u8], cursor: &'b mut Cursor) -> Result<bool, Error> {
    cursor.consume_false(buf)?;
    Ok(false)
}

fn parse_array<'a, 'b>(buf: &'a [u8], cursor: &'b mut Cursor) -> Result<Vec<Value<'a>>, Error> {
    cursor.advance();
    let mut array = Vec::new();
    loop {
        match cursor.next_token(buf) {
            b']' => break,
            _ => {
                array.push(_parse_json(buf, cursor)?);
                match cursor.next_token(buf) {
                    b',' => cursor.advance(),
                    b']' => break,
                    _ => {
                        return Err(Error {
                            pos: cursor.pos,
                            msg: "Unexpected token when parsing array",
                        })
                    }
                }
            }
        }
    }
    cursor.advance();
    Ok(array)
}

fn parse_object<'a, 'b>(
    buf: &'a [u8],
    cursor: &'b mut Cursor,
) -> Result<HashMap<Cow<'a, str>, Value<'a>>, Error> {
    cursor.advance();
    let mut obj = HashMap::new();
    loop {
        match cursor.next_token(buf) {
            b'}' => break,
            _ => {
                let key = parse_str(buf, cursor)?;
                if cursor.next_token(buf) != b':' {
                    return Err(Error {
                        pos: cursor.pos,
                        msg: "Unexpcted object key value separator",
                    });
                }
                cursor.advance();
                let value = _parse_json(buf, cursor)?;
                obj.insert(key, value);
                match cursor.next_token(buf) {
                    b',' => cursor.advance(),
                    b'}' => break,
                    _ => {
                        return Err(Error {
                            pos: cursor.pos,
                            msg: "Unexpected token when parsing object",
                        })
                    }
                }
            }
        }
    }
    cursor.advance();
    Ok(obj)
}

pub fn serialize_json(val: &Value, buf: &mut Vec<u8>) {
    match val {
        Value::Int(v) => write!(buf, "{}", v).unwrap(),
        Value::Bool(v) => write!(buf, "{}", v).unwrap(),
        Value::Float(v) => write!(buf, "{}", v).unwrap(),
        Value::Null(()) => buf.extend_from_slice(b"null"),
        Value::String(v) => serialize_str(v, buf),
        Value::Object(v) => serialize_object(v, buf),
        Value::Array(v) => serialize_array(v, buf),
    }
}

fn serialize_str(s: &Cow<str>, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"\"");
    for c in s.bytes() {
        let s;
        buf.extend_from_slice(match c {
            b'"' => b"\\\"",
            b'\\' => b"\\\\",
            b'/' => b"/",
            b'\n' => b"\\n",
            b'\r' => b"\\r",
            b'\t' => b"\\t",
            0x08 => b"\\b",
            0x0C => b"\\f",
            _ => {
                s = [c];
                &s
            }
        });
    }
    buf.extend_from_slice(b"\"");
}

fn serialize_object(o: &HashMap<Cow<str>, Value>, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"{");
    let mut first = true;
    for (key, val) in o {
        if !first {
            buf.extend_from_slice(b", ");
        }
        first = false;
        serialize_str(key, buf);
        buf.extend_from_slice(b": ");
        serialize_json(val, buf);
    }
    buf.extend_from_slice(b"}");
}

fn serialize_array(a: &Vec<Value>, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"[");
    let mut first = true;
    for val in a {
        if !first {
            buf.extend_from_slice(b", ");
        }
        first = false;
        serialize_json(val, buf);
    }
    buf.extend_from_slice(b"]");
}

#[cfg(test)]
mod test {
    use std::{borrow::Cow, collections::HashMap, str};

    use super::{parse_json, serialize_json, Error, Value};

    #[test]
    fn test_parse_simple_values() {
        let cases: [(&[u8], Value); 13] = [
            (b"\"hello there\"", Value::String("hello there".into())),
            (
                b"   \n\r   \"hello there\"  ",
                Value::String("hello there".into()),
            ),
            (
                b"\"General Kenobi \\\\\"",
                Value::String("General Kenobi \\".into()),
            ),
            (b"\"\"", Value::String("".into())),
            (b"1", Value::Int(1)),
            (b"314", Value::Int(314)),
            (b"3.14", Value::Float(3.14)),
            (b"true", Value::Bool(true)),
            (b"false", Value::Bool(false)),
            (b"null", Value::Null(())),
            (
                b"{\"a\": null, \n \"b\": [], \"c\"  : {}}",
                Value::Object(
                    [
                        (Cow::Borrowed("a"), Value::Null(())),
                        (Cow::Borrowed("b"), Value::Array(Vec::new())),
                        (Cow::Borrowed("c"), Value::Object(HashMap::new())),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ),
            (
                b"{\"method\": \"isPrime\", \"number\": 123}",
                Value::Object(
                    [
                        (
                            Cow::Borrowed("method"),
                            Value::String(Cow::Borrowed("isPrime")),
                        ),
                        (Cow::Borrowed("number"), Value::Int(123)),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ),
            (
                b"[\"\", 3.14, 314]",
                Value::Array(vec![
                    Value::String("".into()),
                    Value::Float(3.14),
                    Value::Int(314),
                ]),
            ),
        ];
        for (input, expected) in cases {
            let val = parse_json(input).expect("parsing failed");
            assert_eq!(val, expected);
        }
    }

    #[test]
    fn test_parse_string_interrupted() -> Result<(), Error> {
        parse_json("\"hello there ".as_bytes()).err().unwrap();
        Ok(())
    }

    #[test]
    fn test_parse_errors() {
        let inputs = [
            b"{".as_ref(),
            b"[,]",
            b"\"hello there",
            b"hey",
            b"",
            b"{1: 1}",
            b"{1: [}}",
        ];
        for input in inputs {
            parse_json(input).unwrap_err();
        }
    }

    #[test]
    fn test_serialize_simples_values() {
        let cases = [
            (Value::Int(123), "123"),
            (Value::Int(-123), "-123"),
            (Value::Bool(false), "false"),
            (Value::Bool(true), "true"),
            (Value::Float(0.123), "0.123"),
            (Value::Float(f64::NAN), "NaN"),
            (Value::Null(()), "null"),
            (Value::String("foo".into()), "\"foo\""),
            (Value::String("\"".into()), "\"\\\"\""),
            (Value::String("\\".into()), "\"\\\\\""),
            (
                Value::Object(
                    [(Cow::Borrowed("a"), Value::Null(()))]
                        .into_iter()
                        .collect(),
                ),
                "{\"a\": null}",
            ),
            (Value::Array(vec![Value::Null(())]), "[null]"),
        ];

        for (input, expected) in cases {
            let mut buf = Vec::new();
            serialize_json(&input, &mut buf);
            let res = str::from_utf8(&buf).expect("serialized to non utf8");
            assert_eq!(res, expected);
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let inputs = [
            Value::Object(
                [
                    (Cow::Borrowed("a"), Value::Null(())),
                    (Cow::Borrowed("b"), Value::Object(HashMap::new())),
                    (Cow::Borrowed("c"), Value::Array(Vec::new())),
                ]
                .into_iter()
                .collect(),
            ),
            Value::Array(vec![
                Value::String("".into()),
                Value::Float(3.14),
                Value::Int(314),
            ]),
        ];

        for input in inputs {
            let mut buf = Vec::new();
            serialize_json(&input, &mut buf);
            let res = parse_json(&buf).expect("Couldn't parse output");
            assert_eq!(res, input);
        }
    }
}
