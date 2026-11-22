pub(crate) struct Comment {
    pub(crate) line: usize,
    pub(crate) text: String,
}

pub(crate) struct Scanned {
    pub(crate) comments: Vec<Comment>,
    pub(crate) code_lines: Vec<String>,
}

impl Scanned {
    pub(crate) fn comment_on(&self, line: usize) -> Option<&Comment> {
        self.comments.iter().find(|comment| comment.line == line)
    }

    pub(crate) fn code_on(&self, line: usize) -> &str {
        line.checked_sub(1)
            .and_then(|index| self.code_lines.get(index))
            .map_or("", String::as_str)
    }

    pub(crate) fn keyword_lines(&self, keyword: &str) -> Vec<usize> {
        self.code_lines
            .iter()
            .enumerate()
            .filter(|(_, code)| contains_word(code, keyword))
            .map(|(index, _)| index + 1)
            .collect()
    }
}

pub(crate) fn scan(source: &str) -> Scanned {
    let mut scanner = Scanner {
        input: source.chars().collect(),
        position: 0,
        line: 1,
        code: String::new(),
        comments: Vec::new(),
    };
    scanner.run();
    Scanned {
        comments: scanner.comments,
        code_lines: scanner.code.split('\n').map(str::to_owned).collect(),
    }
}

struct Scanner {
    input: Vec<char>,
    position: usize,
    line: usize,
    code: String,
    comments: Vec<Comment>,
}

impl Scanner {
    fn run(&mut self) {
        while let Some(character) = self.peek(0) {
            match (character, self.peek(1)) {
                ('/', Some('/')) => self.line_comment(),
                ('/', Some('*')) => self.block_comment(),
                ('"', _) => self.string(),
                ('\'', _) => self.quote(),
                _ => self.keep(),
            }
        }
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.input.get(self.position + ahead).copied()
    }

    fn keep(&mut self) {
        if let Some(character) = self.peek(0) {
            self.code.push(character);
            self.advance();
        }
    }

    fn blank(&mut self) -> Option<char> {
        let character = self.peek(0)?;
        self.code.push(if character == '\n' { '\n' } else { ' ' });
        self.advance();
        Some(character)
    }

    fn advance(&mut self) {
        if self.peek(0) == Some('\n') {
            self.line += 1;
        }
        self.position += 1;
    }

    fn line_comment(&mut self) {
        let line = self.line;
        let mut text = String::new();
        while let Some(character) = self.peek(0) {
            if character == '\n' {
                break;
            }
            text.push(character);
            self.blank();
        }
        self.comments.push(Comment { line, text });
    }

    fn block_comment(&mut self) {
        let line = self.line;
        let mut text = String::new();
        let mut depth = 0usize;
        while let Some(character) = self.peek(0) {
            let pair = (character, self.peek(1));
            if matches!(pair, ('/', Some('*')) | ('*', Some('/'))) {
                text.push(character);
                self.blank();
                text.extend(self.blank());
                if pair.0 == '/' {
                    depth += 1;
                } else {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
            } else {
                text.push(character);
                self.blank();
            }
        }
        self.comments.push(Comment { line, text });
    }

    fn string(&mut self) {
        let raw_hashes = self.raw_string_hashes();
        self.keep();
        let hashes = raw_hashes.unwrap_or(0);
        while let Some(character) = self.peek(0) {
            if raw_hashes.is_none() && character == '\\' {
                self.blank();
                self.blank();
            } else if character == '"' && (1..=hashes).all(|ahead| self.peek(ahead) == Some('#')) {
                for _ in 0..=hashes {
                    self.keep();
                }
                return;
            } else {
                self.blank();
            }
        }
    }

    fn raw_string_hashes(&self) -> Option<usize> {
        let prefix = self.code.trim_end_matches('#');
        let hashes = self.code.len() - prefix.len();
        let before_r = prefix.strip_suffix('r')?;
        let before_prefix = before_r.strip_suffix(['b', 'c']).unwrap_or(before_r);
        let is_identifier = before_prefix
            .chars()
            .next_back()
            .is_some_and(|character| character.is_alphanumeric() || character == '_');
        (!is_identifier).then_some(hashes)
    }

    fn quote(&mut self) {
        let is_char = matches!(
            (self.peek(1), self.peek(2)),
            (Some('\\'), _) | (Some(_), Some('\''))
        );
        self.keep();
        if !is_char {
            return;
        }
        while let Some(character) = self.peek(0) {
            if character == '\\' {
                self.blank();
                self.blank();
            } else if character == '\'' {
                self.keep();
                return;
            } else {
                self.blank();
            }
        }
    }
}

fn contains_word(code: &str, word: &str) -> bool {
    let is_identifier = |character: char| character.is_alphanumeric() || character == '_';
    code.match_indices(word).any(|(start, _)| {
        let before = code.get(..start).and_then(|text| text.chars().next_back());
        let after = code
            .get(start + word.len()..)
            .and_then(|text| text.chars().next());
        !before.is_some_and(is_identifier) && !after.is_some_and(is_identifier)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_comments_but_not_comment_markers_in_strings() {
        let scanned = scan("let url = \"https://example\";\n// note\nlet raw = r#\"// \"#;\n");
        assert_eq!(scanned.comments.len(), 1);
        assert_eq!(scanned.comments[0].line, 2);
        assert_eq!(scanned.comments[0].text, "// note");
    }

    #[test]
    fn finds_keywords_outside_strings_and_identifiers() {
        let scanned =
            scan("let text = \"unsafe\";\nlet unsafe_code = 1;\nunsafe { run() }\nlet c = '\"';\n");
        assert_eq!(scanned.keyword_lines("unsafe"), [3]);
    }

    #[test]
    fn lifetimes_are_not_char_literals() {
        let scanned = scan("fn f<'a>(x: &'a str) {}\n// end\n");
        assert_eq!(scanned.comments.len(), 1);
    }
}
