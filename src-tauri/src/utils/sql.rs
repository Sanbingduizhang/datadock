//! SQL 文本处理。
//!
//! 阶段 E 第一步只支持单语句执行，但切分逻辑必须正确识别引号与注释，
//! 不能简单按分号切割（见 `docs/13` 阶段 E 要求）。

/// 将 SQL 文本按语句切分（去除空语句与注释行）。
///
/// 已处理：单引号、双引号、反引号转义，`--` / `#` 行注释，`/* */` 块注释。
/// 存储过程的 `DELIMITER` 变更暂不支持，遇到时会整体视为一条语句。
pub fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();

    #[derive(PartialEq)]
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        Backtick,
        LineComment,
        BlockComment,
    }

    let mut state = State::Normal;

    while let Some(ch) = chars.next() {
        match state {
            State::Normal => match ch {
                '\'' => {
                    state = State::SingleQuote;
                    current.push(ch);
                }
                '"' => {
                    state = State::DoubleQuote;
                    current.push(ch);
                }
                '`' => {
                    state = State::Backtick;
                    current.push(ch);
                }
                '-' if chars.peek() == Some(&'-') => {
                    state = State::LineComment;
                    chars.next();
                }
                '#' => {
                    state = State::LineComment;
                }
                '/' if chars.peek() == Some(&'*') => {
                    state = State::BlockComment;
                    chars.next();
                }
                ';' => {
                    push_statement(&mut statements, &mut current);
                }
                _ => current.push(ch),
            },
            State::SingleQuote => {
                current.push(ch);
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == '\'' {
                    state = State::Normal;
                }
            }
            State::DoubleQuote => {
                current.push(ch);
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == '"' {
                    state = State::Normal;
                }
            }
            State::Backtick => {
                current.push(ch);
                if ch == '`' {
                    state = State::Normal;
                }
            }
            State::LineComment => {
                if ch == '\n' {
                    state = State::Normal;
                    current.push('\n');
                }
            }
            State::BlockComment => {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    state = State::Normal;
                    current.push(' ');
                }
            }
        }
    }

    push_statement(&mut statements, &mut current);
    statements
}

fn push_statement(statements: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_statements() {
        let statements = split_statements("SELECT 1; SELECT 2;");

        assert_eq!(statements, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn ignores_trailing_semicolon() {
        assert_eq!(split_statements("SELECT 1;  "), vec!["SELECT 1"]);
    }

    #[test]
    fn keeps_semicolon_inside_strings() {
        let statements = split_statements("INSERT INTO t VALUES ('a;b'); SELECT 2");

        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "INSERT INTO t VALUES ('a;b')");
    }

    #[test]
    fn handles_escaped_quote() {
        let statements = split_statements("SELECT 'it\\'s; ok'; SELECT 2");

        assert_eq!(statements.len(), 2);
    }

    #[test]
    fn skips_line_comments() {
        let statements = split_statements("-- comment; with semicolon\nSELECT 1");

        assert_eq!(statements, vec!["SELECT 1"]);
    }

    #[test]
    fn skips_block_comments() {
        let statements = split_statements("/* a;b */ SELECT 1");

        assert_eq!(statements, vec!["SELECT 1"]);
    }

    #[test]
    fn handles_backtick_identifiers() {
        let statements = split_statements("SELECT * FROM `we;ird` WHERE id = 1");

        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("`we;ird`"));
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(split_statements("   \n -- only comment\n").is_empty());
    }
}
