//! An evaluator of arithmetic expressions for parametric dimensions.
//!
//! Supported: `+ - * / ^`, unary minus, parentheses, the functions sin, cos, tan, sqrt, abs, min, max, floor,
//! ceil and round, the constants pi, tau and e, and parameter names. Trigonometric angles are in degrees, as
//! they are in sketches. Returns either a value or a readable error.

use std::collections::HashMap;

use crate::errors::ExprError;

/// Evaluate the expression `src`, substituting parameters from `vars`.
///
/// A parameter name is matched exactly: `H` and `h` are two parameters. The names of the functions and constants
/// are not, `SIN(30)` and `PI` read as `sin(30)` and `pi`; a parameter named `E` or `Pi` is still found under its own
/// name, since the constant gives way to it in any case but its own.
pub fn eval(src: &str, vars: &HashMap<String, f64>) -> Result<f64, ExprError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks: &toks, pos: 0, vars, depth: 0 };
    let v = p.expr()?;
    if p.pos != p.toks.len() {
        return Err(ExprError::TrailingInput(p.peek_str()));
    }
    if !v.is_finite() {
        return Err(ExprError::NotANumber);
    }
    Ok(v)
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, ExprError> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                out.push(Tok::Caret);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                    i += 1;
                }
                let txt: String = cs[start..i].iter().collect();
                let v: f64 = txt.parse().map_err(|_| ExprError::UnknownChar(txt.clone()))?;
                out.push(Tok::Num(v));
            }
            _ if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                let txt: String = cs[start..i].iter().collect();
                out.push(Tok::Ident(txt));
            }
            _ => return Err(ExprError::UnknownChar(c.to_string())),
        }
    }
    Ok(out)
}

/// THE DEEPEST A FORMULA MAY NEST - parentheses, function calls, signs and powers inside one another, counted
/// together, so `-(` is two levels. The parser goes down one call per level (`nest`), and nothing stopped it:
/// measured (release build), a formula of 4,375 nested parentheses (8,751 characters) ran a 2 MiB thread's stack
/// out, and 17,628 the 8 MiB main thread on Linux, and the program died - on the interface thread, as the parameters
/// window, an undo or a sketch solve evaluated a formula stored in a document. Formulas people write nest a few
/// levels; this leaves room for generated ones, and a formula at the limit is read on a 256 KiB thread in a debug
/// build - a quarter of the 1 MiB main thread Windows gives, the rest left to the drawing it is evaluated in.
pub const MAX_FORMULA_DEPTH: usize = 256;

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    vars: &'a HashMap<String, f64>,
    /// how many levels the parse is inside, see `MAX_FORMULA_DEPTH`
    depth: usize,
}

impl<'a> Parser<'a> {
    /// One level deeper: every way the parser calls itself again goes through here, so the depth is counted the
    /// same for all of them, and a formula past `MAX_FORMULA_DEPTH` is refused rather than allowed to run the stack
    /// out.
    fn nest<T>(&mut self, go: impl FnOnce(&mut Self) -> Result<T, ExprError>) -> Result<T, ExprError> {
        if self.depth >= MAX_FORMULA_DEPTH {
            return Err(ExprError::TooDeep { limit: MAX_FORMULA_DEPTH });
        }
        self.depth += 1;
        let out = go(self);
        self.depth -= 1;
        out
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn peek_str(&self) -> String {
        match self.peek() {
            Some(Tok::Num(n)) => n.to_string(),
            Some(Tok::Ident(s)) => s.clone(),
            Some(t) => tok_text(t),
            // end of input: an empty string rather than a word, since the application supplies the words
            None => String::new(),
        }
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    // expr := term (('+'|'-') term)*
    fn expr(&mut self) -> Result<f64, ExprError> {
        let mut v = self.term()?;
        while let Some(t) = self.peek() {
            match t {
                Tok::Plus => {
                    self.bump();
                    v += self.term()?;
                }
                Tok::Minus => {
                    self.bump();
                    v -= self.term()?;
                }
                _ => break,
            }
        }
        Ok(v)
    }

    // term := power (('*'|'/') power)*
    fn term(&mut self) -> Result<f64, ExprError> {
        let mut v = self.power()?;
        while let Some(t) = self.peek() {
            match t {
                Tok::Star => {
                    self.bump();
                    v *= self.power()?;
                }
                Tok::Slash => {
                    self.bump();
                    v /= self.power()?;
                }
                _ => break,
            }
        }
        Ok(v)
    }

    // power := unary ('^' power)?   right-associative
    fn power(&mut self) -> Result<f64, ExprError> {
        let b = self.unary()?;
        if matches!(self.peek(), Some(Tok::Caret)) {
            self.bump();
            let e = self.nest(|p| p.power())?;
            return Ok(b.powf(e));
        }
        Ok(b)
    }

    // unary := ('-'|'+')? atom
    fn unary(&mut self) -> Result<f64, ExprError> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.bump();
                Ok(-self.nest(|p| p.unary())?)
            }
            Some(Tok::Plus) => {
                self.bump();
                self.nest(|p| p.unary())
            }
            _ => self.atom(),
        }
    }

    // atom := Num | '(' expr ')' | Ident | Ident '(' args ')'
    fn atom(&mut self) -> Result<f64, ExprError> {
        match self.bump() {
            Some(Tok::Num(n)) => Ok(n),
            Some(Tok::LParen) => {
                let v = self.nest(|p| p.expr())?;
                if !matches!(self.bump(), Some(Tok::RParen)) {
                    return Err(ExprError::ExpectedParen);
                }
                Ok(v)
            }
            Some(Tok::Ident(name)) => {
                if matches!(self.peek(), Some(Tok::LParen)) {
                    self.bump(); // (
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        args.push(self.nest(|p| p.expr())?);
                        while matches!(self.peek(), Some(Tok::Comma)) {
                            self.bump();
                            args.push(self.nest(|p| p.expr())?);
                        }
                    }
                    if !matches!(self.bump(), Some(Tok::RParen)) {
                        return Err(ExprError::ExpectedParenAfterArgs);
                    }
                    call_fn(&name.to_lowercase(), &args)
                } else {
                    // Without parentheses this is a name, not a function. `UnknownFn` used to be returned
                    // here, so a bare `w` in a formula answered "unknown function: w" — advice about the wrong
                    // thing, since a parameter was meant.
                    constant(&name).or_else(|| self.vars.get(&name).copied()).or_else(|| constant(&name.to_lowercase())).ok_or_else(|| ExprError::UnknownName(name.clone()))
                }
            }
            // The end of input is not a token. `format!("{other:?}")` on `None` produced "Unexpected token
            // None": a Rust debug name instead of an explanation.
            None => Err(ExprError::UnexpectedEnd),
            Some(t) => Err(ExprError::UnexpectedToken(tok_text(&t))),
        }
    }
}

/// A token as it was actually typed.
///
/// An error message shows the input itself rather than the name of an enum variant: "trailing input: )" reads,
/// "trailing input: RParen" does not.
fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Num(n) => n.to_string(),
        Tok::Ident(s) => s.clone(),
        Tok::Plus => "+".into(),
        Tok::Minus => "-".into(),
        Tok::Star => "*".into(),
        Tok::Slash => "/".into(),
        Tok::Caret => "^".into(),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::Comma => ",".into(),
    }
}

/// Whether a formula reads `name` as a constant rather than as a parameter. Only the exact lowercase spelling:
/// a parameter `E` or `Pi` is found under its own name before the case is folded.
pub fn is_constant(name: &str) -> bool {
    constant(name.trim()).is_some()
}

fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" => Some(std::f64::consts::PI),
        "tau" => Some(std::f64::consts::TAU),
        "e" => Some(std::f64::consts::E),
        _ => None,
    }
}

fn call_fn(name: &str, args: &[f64]) -> Result<f64, ExprError> {
    let one = |f: fn(f64) -> f64| -> Result<f64, ExprError> {
        if args.len() != 1 {
            return Err(ExprError::NeedsOneArg(name.to_string()));
        }
        Ok(f(args[0]))
    };
    match name {
        // trigonometry in degrees
        "sin" => one(|x| x.to_radians().sin()),
        "cos" => one(|x| x.to_radians().cos()),
        "tan" => one(|x| x.to_radians().tan()),
        "asin" => one(|x| x.asin().to_degrees()),
        "acos" => one(|x| x.acos().to_degrees()),
        "atan" => one(|x| x.atan().to_degrees()),
        "sqrt" => one(|x| x.sqrt()),
        "abs" => one(|x| x.abs()),
        "floor" => one(|x| x.floor()),
        "ceil" => one(|x| x.ceil()),
        "round" => one(|x| x.round()),
        "ln" => one(|x| x.ln()),
        "min" => {
            if args.len() != 2 {
                return Err(ExprError::NeedsTwoArgs("min".into()));
            }
            Ok(args[0].min(args[1]))
        }
        "max" => {
            if args.len() != 2 {
                return Err(ExprError::NeedsTwoArgs("max".into()));
            }
            Ok(args[0].max(args[1]))
        }
        _ => Err(ExprError::UnknownFn(name.to_string())),
    }
}

/// Whether the parameter `name` is mentioned in an expression as an identifier of its own rather than as part
/// of another name — `L` must not be found inside `Length`. Needed for targeted rebuilds: editing a parameter
/// marks dirty only the features that actually reference it.
/// A number as a string, written the way a person writes it: at most four decimals, no trailing zeros and no
/// dot at the end.
///
/// One rule for the whole project. Automatically generated values in input fields carried far too many
/// decimals; four are enough, and the rule applies to every field. A bare `format!("{v}")` prints the whole
/// truth about an `f64` — `12.750000000000002` — leaving the tail to be cleaned up by hand. Four decimals is a
/// micron on a metre-long part, and there is nothing to show beyond that.
///
/// What a person typed does not pass through this door: their text is their own.
pub fn fmt_num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    // A minus sign on zero is not shown: it comes from the sign of a small quantity that this very code
    // rounded away, and reads as a negative zero, that is, as an error.
    if s.is_empty() || s == "-" || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

pub fn mentions(expr: &str, name: &str) -> bool {
    !occurrences(expr, name).is_empty()
}

/// The places where `name` stands in an expression as a name rather than as part of another word. Both
/// [`mentions`] and [`rename_ident`] grow from here: the boundary rule has to be single, or what gets renamed
/// is not what was found.
fn occurrences(expr: &str, name: &str) -> Vec<usize> {
    let mut out = Vec::new();
    if name.is_empty() {
        return out;
    }
    let bytes = expr.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80;
    let mut from = 0;
    while let Some(pos) = expr[from..].find(name) {
        let i = from + pos;
        let j = i + name.len();
        let left_ok = i == 0 || !ident(bytes[i - 1]);
        let right_ok = j >= bytes.len() || !ident(bytes[j]);
        if left_ok && right_ok {
            out.push(i);
            // The next place begins after this one. Places that overlap - `a+a` twice in `a+a+a`, a name only a
            // project file can hold - would have a rename cut the text backwards, from the end of one to the start
            // of the next. A name of letters, digits and `_` cannot overlap itself, so for those nothing changes.
            from = j;
            continue;
        }
        // The step advances by a whole character rather than by a byte. `from = i + 1` in the middle of a
        // multi-byte letter crashes the parse; the same fault lived in the earlier `mentions` and surfaced only
        // when the first match was rejected by the boundary check.
        from = i + expr[i..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

/// Replace a name throughout an expression without touching other words.
///
/// Renaming a parameter has to reach the formulas: otherwise renaming `w` to `shirina` leaves expressions such
/// as `w*2+5` across the project, pointing at a name that no longer exists, and the model breaks silently. The
/// boundaries are those of [`mentions`]: a `w` inside `wall`, `pow` or `w2` is not a name and is left alone.
pub fn rename_ident(expr: &str, old: &str, new: &str) -> String {
    let at = occurrences(expr, old);
    if at.is_empty() {
        return expr.to_string();
    }
    let mut out = String::with_capacity(expr.len() + at.len() * new.len());
    let mut cut = 0;
    for i in at {
        out.push_str(&expr[cut..i]);
        out.push_str(new);
        cut = i + old.len();
    }
    out.push_str(&expr[cut..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        eval(s, &HashMap::new()).unwrap()
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert!((ev("2 + 3 * 4") - 14.0).abs() < 1e-9);
        assert!((ev("(2 + 3) * 4") - 20.0).abs() < 1e-9);
        assert!((ev("2 ^ 3 ^ 2") - 512.0).abs() < 1e-9); // right-associative
        assert!((ev("-5 + 2") + 3.0).abs() < 1e-9);
        assert!((ev("10 / 4") - 2.5).abs() < 1e-9);
    }

    #[test]
    fn functions_and_constants() {
        assert!((ev("sqrt(16)") - 4.0).abs() < 1e-9);
        assert!((ev("sin(90)") - 1.0).abs() < 1e-9); // degrees
        assert!((ev("max(3, 7)") - 7.0).abs() < 1e-9);
        assert!((ev("2 * pi") - std::f64::consts::TAU).abs() < 1e-9);
        assert!((ev("SQRT(16) + Max(1, 2)") - 6.0).abs() < 1e-9, "a function is read in any case");
        assert!((ev("2 * PI") - std::f64::consts::TAU).abs() < 1e-9, "a constant is read in any case");
    }

    /// A parameter named like a constant in another case is found under its own name: `E` is the parameter,
    /// `e` the constant.
    #[test]
    fn a_parameter_is_not_shadowed_by_a_constant_in_another_case() {
        let mut v = HashMap::new();
        v.insert("E".to_string(), 5.0);
        assert!((eval("E", &v).unwrap() - 5.0).abs() < 1e-9);
        assert!((eval("e", &v).unwrap() - std::f64::consts::E).abs() < 1e-9);
    }

    /// Names that differ only in case are two parameters.
    #[test]
    fn a_name_is_matched_in_its_own_case() {
        let mut v = HashMap::new();
        v.insert("H".to_string(), 40.0);
        v.insert("h".to_string(), 20.0);
        assert!((eval("H - h", &v).unwrap() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn parameters() {
        let mut v = HashMap::new();
        v.insert("w".to_string(), 50.0);
        v.insert("h".to_string(), 20.0);
        assert!((eval("w / 2", &v).unwrap() - 25.0).abs() < 1e-9);
        assert!((eval("w + h", &v).unwrap() - 70.0).abs() < 1e-9);
        assert!(eval("W*2", &v).is_err(), "W is not the parameter w");
    }

    #[test]
    fn errors() {
        assert!(eval("w + 1", &HashMap::new()).is_err()); // an unknown parameter
        assert!(eval("2 +", &HashMap::new()).is_err());
        assert!(eval("(2", &HashMap::new()).is_err());
        assert!(eval("1/0", &HashMap::new()).is_err()); // not a number
    }
}
