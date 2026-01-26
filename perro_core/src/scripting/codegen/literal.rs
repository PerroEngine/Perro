// Literal code generation
use crate::scripting::ast::{Literal, NumberKind, Type};
use regex::Regex;

impl Literal {
    pub(crate) fn to_rust(&self, expected_type: Option<&Type>) -> String {
        match self {
            Literal::Number(raw) => match expected_type {
                Some(Type::Number(NumberKind::Signed(w))) => format!("{}i{}", raw, w),
                Some(Type::Number(NumberKind::Unsigned(w))) => format!("{}u{}", raw, w),
                Some(Type::Number(NumberKind::Float(w))) => match w {
                    32 => format!("{}f32", raw),
                    64 => format!("{}f64", raw),
                    _ => format!("{}f32", raw),
                },
                Some(Type::Number(NumberKind::Decimal)) => {
                    format!("Decimal::from_str(\"{}\").unwrap()", raw)
                }
                Some(Type::Number(NumberKind::BigInt)) => {
                    format!("BigInt::from_str(\"{}\").unwrap()", raw)
                }
                _ => format!("{}f32", raw),
            },

            Literal::String(s) => {
                match expected_type {
                    // For Cow<'static, str>, use Cow::Borrowed with string literal
                    Some(Type::CowStr) => {
                        format!("Cow::Borrowed(\"{}\")", s)
                    }
                    // For Option<CowStr>, use Some(Cow::Borrowed(...))
                    Some(Type::Option(inner)) if matches!(inner.as_ref(), Type::CowStr) => {
                        format!("Some(Cow::Borrowed(\"{}\"))", s)
                    }
                    // For StrRef (&str), use string literal
                    Some(Type::StrRef) => format!("\"{}\"", s),
                    // For String or unknown, create owned String
                    _ => format!("String::from(\"{}\")", s),
                }
            }

            Literal::Bool(b) => b.to_string(),

            Literal::Null => {
                // null literal converts to None for Option<T> types
                match expected_type {
                    Some(Type::Option(_)) => "None".to_string(),
                    _ => "None".to_string(), // Default to None, will be type-checked elsewhere
                }
            }

            Literal::Interpolated(s) => {
                let re = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
                let mut fmt = String::new();
                let mut args = Vec::new();
                let mut last = 0;

                for cap in re.captures_iter(s) {
                    let m = cap.get(0).unwrap();
                    fmt.push_str(&s[last..m.start()]);
                    fmt.push_str("{}");
                    last = m.end();
                    args.push(cap[1].to_string());
                }
                fmt.push_str(&s[last..]);

                if args.is_empty() {
                    format!("\"{}\"", fmt)
                } else {
                    format!("format!(\"{}\", {})", fmt, args.join(", "))
                }
            }
        }
    }
}
