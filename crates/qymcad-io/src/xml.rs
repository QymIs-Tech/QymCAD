//! A SMALL XML READER for the formats that are XML - 3MF and AMF.
//!
//! Written here rather than taken as a library: the one the tree already carries comes in through the SVG
//! reader and is not a direct dependency, and making it one is a dependency to watch and to answer for. These
//! files are written by machines, and what they use of XML is small: elements, attributes in either quotes,
//! the five named entities and numeric ones, text, comments, processing instructions, a doctype, CDATA. Names are
//! compared by their local part, so `m:object` and `object` meet - the formats keep their own elements under
//! names of their own.

/// An element: its local name, its attributes, its children, and the text directly inside it.
#[derive(Debug, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
    /// The line of the file its opening tag stands on, from 1: a refusal says where to look.
    pub line: usize,
}

impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| local(k) == name).map(|(_, v)| v.as_str())
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// Every element under this one, at any depth, in document order.
    pub fn descendants(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut stack: Vec<&Node> = self.children.iter().rev().collect();
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(n.children.iter().rev());
        }
        out
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn unescape(s: &str) -> Option<String> {
    if !s.contains('&') {
        return Some(s.to_string());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find(';')? + i;
        let ent = &rest[i + 1..end];
        match ent {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(h) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) { u32::from_str_radix(h, 16).ok()? } else { ent.strip_prefix('#')?.parse().ok()? };
                out.push(char::from_u32(code)?);
            }
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Text for an attribute of an XML being written: the characters XML gives a meaning, as their named entities.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Parse a document into its root element. `None` for anything that is not well-formed enough to read.
pub fn parse(text: &str) -> Option<Node> {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut stack: Vec<Node> = vec![Node::default()]; // a holder for the root
    let (mut counted, mut line) = (0usize, 1usize); // the line at byte `counted`, carried forward to each element
    while i < b.len() {
        if b[i] != b'<' {
            let end = text[i..].find('<').map(|k| i + k).unwrap_or(b.len());
            let t = unescape(&text[i..end])?;
            stack.last_mut()?.text.push_str(&t);
            i = end;
            continue;
        }
        let rest = &text[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->")? + 3;
        } else if rest.starts_with("<![CDATA[") {
            let end = rest.find("]]>")?;
            stack.last_mut()?.text.push_str(&rest[9..end]);
            i += end + 3;
        } else if rest.starts_with("<?") {
            i += rest.find("?>")? + 2;
        } else if rest.starts_with("<!") {
            i += rest.find('>')? + 1; // a doctype; an internal subset with its own brackets is not written by these formats
        } else if rest.starts_with("</") {
            let end = rest.find('>')?;
            let name = local(rest[2..end].trim());
            let done = stack.pop()?;
            if done.name != name || stack.is_empty() {
                return None;
            }
            stack.last_mut()?.children.push(done);
            i += end + 1;
        } else {
            // an opening tag: its name, then attributes up to `>` or `/>`
            let mut k = i + 1;
            while k < b.len() && !b[k].is_ascii_whitespace() && b[k] != b'>' && b[k] != b'/' {
                k += 1;
            }
            line += text[counted..i].matches('\n').count();
            counted = i;
            // every field named, none taken from `Node::default()`: the form stays good if `Node` gets a `Drop` of its own
            let mut node = Node { name: local(&text[i + 1..k]).to_string(), attrs: Vec::new(), children: Vec::new(), text: String::new(), line };
            loop {
                while k < b.len() && b[k].is_ascii_whitespace() {
                    k += 1;
                }
                match b.get(k)? {
                    b'>' => {
                        stack.push(node);
                        k += 1;
                        break;
                    }
                    b'/' if b.get(k + 1) == Some(&b'>') => {
                        stack.last_mut()?.children.push(node);
                        k += 2;
                        break;
                    }
                    _ => {
                        let eq = text[k..].find('=')? + k;
                        let key = text[k..eq].trim().to_string();
                        let mut q = eq + 1;
                        while q < b.len() && b[q].is_ascii_whitespace() {
                            q += 1;
                        }
                        let quote = *b.get(q)?;
                        if quote != b'"' && quote != b'\'' {
                            return None;
                        }
                        let close = text[q + 1..].find(quote as char)? + q + 1;
                        node.attrs.push((key, unescape(&text[q + 1..close])?));
                        k = close + 1;
                    }
                }
            }
            i = k;
        }
    }
    let mut holder = stack.pop()?;
    if !stack.is_empty() || holder.children.len() != 1 {
        return None;
    }
    holder.children.pop()
}
