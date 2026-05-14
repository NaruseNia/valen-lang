//! Comment extraction from Valen source text.
//!
//! Scans source for line (`//`) and block (`/* */`) comments, returning
//! their byte positions so the printer can interleave them into the
//! formatted output.

/// A comment extracted from source text.
#[derive(Debug, Clone)]
pub struct Comment {
    pub text: String,
    pub start: u32,
    pub end: u32,
    pub kind: CommentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    /// `// ...`
    Line,
    /// `/* ... */`
    Block,
}

/// Extract all comments from `source`, preserving their byte positions.
pub fn extract_comments(source: &str) -> Vec<Comment> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut comments = Vec::new();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            // Skip string literals to avoid false positives
            b'"' => {
                i += 1;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            // Skip char literals
            b'\'' => {
                i += 1;
                while i < len && bytes[i] != b'\'' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            b'/' if i + 1 < len => {
                match bytes[i + 1] {
                    // Line comment
                    b'/' => {
                        let start = i;
                        while i < len && bytes[i] != b'\n' {
                            i += 1;
                        }
                        comments.push(Comment {
                            text: source[start..i].to_owned(),
                            start: start as u32,
                            end: i as u32,
                            kind: CommentKind::Line,
                        });
                    }
                    // Block comment
                    b'*' => {
                        let start = i;
                        i += 2;
                        while i + 1 < len {
                            if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                        if i >= len && !(i >= 2 && bytes[i - 2] == b'*' && bytes[i - 1] == b'/') {
                            i = len;
                        }
                        comments.push(Comment {
                            text: source[start..i].to_owned(),
                            start: start as u32,
                            end: i as u32,
                            kind: CommentKind::Block,
                        });
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    comments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_comment() {
        let src = "// hello\nlet x = 1;";
        let comments = extract_comments(src);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "// hello");
        assert_eq!(comments[0].kind, CommentKind::Line);
        assert_eq!(comments[0].start, 0);
    }

    #[test]
    fn block_comment() {
        let src = "/* block */\nlet x = 1;";
        let comments = extract_comments(src);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "/* block */");
        assert_eq!(comments[0].kind, CommentKind::Block);
    }

    #[test]
    fn comment_inside_string_ignored() {
        let src = r#"let x = "// not a comment";"#;
        let comments = extract_comments(src);
        assert!(comments.is_empty());
    }

    #[test]
    fn multiple_comments() {
        let src = "// first\n// second\nlet x = 1;";
        let comments = extract_comments(src);
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "// first");
        assert_eq!(comments[1].text, "// second");
    }
}
