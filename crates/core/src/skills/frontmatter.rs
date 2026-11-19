//! Minimal `SKILL.md` front-matter reader. Skills are untrusted input: this only extracts top-level
//! scalar fields; it never evaluates anything. Supports plain, quoted, and `|` / `>` block scalars.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    pub fields: BTreeMap<String, String>,
    /// The Markdown after the closing `---` (or the whole file if there is no front matter).
    pub body: String,
}

impl Frontmatter {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.fields
            .get(k)
            .map(String::as_str)
            .filter(|v| !v.trim().is_empty())
    }
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return v[1..v.len() - 1].to_string();
        }
    }
    v.to_string()
}

pub fn parse(text: &str) -> Frontmatter {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    // The front matter counts only when the very first line is `---`.
    if lines.next().map(str::trim_end) != Some("---") {
        return Frontmatter {
            fields: BTreeMap::new(),
            body: text.to_string(),
        };
    }
    let mut yaml: Vec<&str> = vec![];
    let mut closed = false;
    for l in lines.by_ref() {
        if l.trim_end() == "---" {
            closed = true;
            break;
        }
        yaml.push(l);
    }
    if !closed {
        return Frontmatter {
            fields: BTreeMap::new(),
            body: text.to_string(),
        };
    }
    let body = lines.collect::<Vec<_>>().join("\n");

    let mut fields = BTreeMap::new();
    let mut i = 0;
    while i < yaml.len() {
        let line = yaml[i];
        i += 1;
        if line.trim().is_empty()
            || line.trim_start().starts_with('#')
            || line.starts_with([' ', '\t'])
        {
            continue;
        }
        let Some((k, rest)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_string();
        if key.is_empty() || key.contains(' ') {
            continue;
        }
        let rest = rest.trim();
        // Indented lines after the key belong to it.
        let mut cont: Vec<&str> = vec![];
        while i < yaml.len() && (yaml[i].starts_with([' ', '\t']) || yaml[i].trim().is_empty()) {
            cont.push(yaml[i].trim());
            i += 1;
        }
        let value = match rest {
            "|" | "|-" | "|+" => cont.join("\n").trim().to_string(),
            ">" | ">-" | ">+" => cont
                .iter()
                .filter(|l| !l.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" "),
            "" => String::new(), // nested map or list: not needed
            plain => {
                let extra: Vec<&str> = cont.iter().filter(|l| !l.is_empty()).copied().collect();
                if extra.is_empty() {
                    unquote(plain)
                } else {
                    format!("{} {}", unquote(plain), extra.join(" "))
                }
            }
        };
        fields.insert(key, value);
    }
    Frontmatter { fields, body }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_quoted_and_block_scalars() {
        let f = parse("---\nname: my-skill\ndescription: \"Does things: well\"\nallowed-tools: Read Grep\n---\n# Body\ntext");
        assert_eq!(f.get("name"), Some("my-skill"));
        assert_eq!(f.get("description"), Some("Does things: well"));
        assert_eq!(f.get("allowed-tools"), Some("Read Grep"));
        assert_eq!(f.body, "# Body\ntext");
        let f = parse("---\nname: x\ndescription: >\n  folded text\n  continues here\n\nother: |\n  line1\n  line2\n---\n");
        assert_eq!(f.get("description"), Some("folded text continues here"));
        assert_eq!(f.get("other"), Some("line1\nline2"));
        let f = parse("---\nname: x\ndescription: first part\n  second part\n---\n");
        assert_eq!(f.get("description"), Some("first part second part"));
    }

    #[test]
    fn front_matter_only_counts_on_the_first_line() {
        let f = parse("\n---\nname: nope\n---\nbody");
        assert!(f.fields.is_empty() && f.body.contains("name: nope"));
        let f = parse("# Title\n---\nname: nope\n---\n");
        assert!(f.fields.is_empty());
        let f = parse("---\nname: unclosed\nno end");
        assert!(f.fields.is_empty());
    }

    #[test]
    fn nested_maps_comments_bom_and_garbage_are_tolerated() {
        let f = parse("\u{feff}---\n# comment\nname: n\nmetadata:\n  a: b\n  c: d\nhooks:\n  - x\nnot a key line\n: oops\n---\n");
        assert_eq!(f.get("name"), Some("n"));
        assert_eq!(f.fields.get("metadata").map(String::as_str), Some(""));
        assert!(!f.fields.contains_key("a"), "nested keys are not top-level");
        assert!(parse("").fields.is_empty());
        assert!(parse("---\n---\n").fields.is_empty());
    }

    #[test]
    fn empty_value_counts_as_missing() {
        let f = parse("---\nname:\ndescription:   \n---\n");
        assert_eq!(f.get("name"), None);
        assert_eq!(f.get("description"), None);
    }
}
