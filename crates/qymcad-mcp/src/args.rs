//! THE ARGUMENTS OF A CALL, read with the reason a value is refused: the model reads the refusal and calls again, so it
//! names the argument and says what was expected.
use qymcad_core::model::Id;
use serde_json::Value;

/// A number the call must carry.
pub fn number(args: &Value, key: &str) -> Result<f64, String> {
    match args.get(key) {
        Some(v) => v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| format!("`{key}` is a number, not {v}")),
        None => Err(format!("`{key}` is missing")),
    }
}

/// A number the call may leave out.
pub fn number_or(args: &Value, key: &str, default: f64) -> Result<f64, String> {
    if args.get(key).is_none() {
        return Ok(default);
    }
    number(args, key)
}

/// A number that has to be above zero: a size, a radius, a thickness.
pub fn size(args: &Value, key: &str) -> Result<f64, String> {
    let v = number(args, key)?;
    if v <= 0.0 {
        return Err(format!("`{key}` is above zero, not {v}"));
    }
    Ok(v)
}

/// An id of the document - a body, a sketch, a node - as `describe_document` lists them.
pub fn id(args: &Value, key: &str) -> Result<Id, String> {
    match args.get(key) {
        Some(v) => v.as_u64().filter(|x| *x > 0).ok_or_else(|| format!("`{key}` is an id from `describe_document`, not {v}")),
        None => Err(format!("`{key}` is missing")),
    }
}

/// A list of face or edge ids, as `describe_body` lists them; left out, it is empty.
pub fn picks(args: &Value, key: &str) -> Result<Vec<u32>, String> {
    let Some(v) = args.get(key) else { return Ok(Vec::new()) };
    let Some(list) = v.as_array() else { return Err(format!("`{key}` is a list of ids, not {v}")) };
    list.iter().map(|x| x.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0).ok_or_else(|| format!("`{key}` holds ids from `describe_body`, not {x}"))).collect()
}

/// A word the call must carry.
pub fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match args.get(key) {
        Some(v) => v.as_str().ok_or_else(|| format!("`{key}` is text, not {v}")),
        None => Err(format!("`{key}` is missing")),
    }
}

/// A word the call may leave out.
pub fn text_or<'a>(args: &'a Value, key: &str, default: &'a str) -> Result<&'a str, String> {
    if args.get(key).is_none() {
        return Ok(default);
    }
    text(args, key)
}

/// One of a closed set of words, refused with the set named.
pub fn choice<'a>(args: &'a Value, key: &str, words: &[&'a str]) -> Result<&'a str, String> {
    let default = words.first().copied().unwrap_or_default();
    let word = text_or(args, key, default)?;
    words.iter().copied().find(|w| *w == word).ok_or_else(|| format!("`{key}` is one of {words:?}, not `{word}`"))
}

/// A point of a sketch, `[x, y]`.
pub fn point(v: &Value, key: &str) -> Result<qymcad_core::geom::Point2, String> {
    let xy = v.as_array().filter(|a| a.len() == 2).and_then(|a| Some(qymcad_core::geom::Point2::new(a[0].as_f64()?, a[1].as_f64()?)));
    xy.ok_or_else(|| format!("`{key}` holds points as [x, y], not {v}"))
}
