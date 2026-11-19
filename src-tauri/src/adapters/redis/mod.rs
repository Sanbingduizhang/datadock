//! Redis 数据源适配器（命令控制台 + 键浏览）。

pub mod command;
pub mod connection;
pub mod edit;
pub mod keys;

use crate::errors::AppError;

/// 解析命令行参数，支持单/双引号与反斜杠转义。
///
/// 例：`HSET user:1 name "alice smith"` → ["HSET", "user:1", "name", "alice smith"]
pub fn parse_command(line: &str) -> Result<Vec<String>, AppError> {
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut has_token = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Some(q) => {
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    has_token = true;
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                    has_token = true;
                }
                c if c.is_whitespace() => {
                    if has_token {
                        args.push(std::mem::take(&mut current));
                        has_token = false;
                    }
                }
                c => {
                    current.push(c);
                    has_token = true;
                }
            },
        }
    }

    if quote.is_some() {
        return Err(AppError::validation("命令中的引号没有闭合"));
    }
    if has_token {
        args.push(current);
    }

    if args.is_empty() {
        return Err(AppError::validation("请输入 Redis 命令"));
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_plain_command() {
        assert_eq!(
            parse_command("GET user:1").expect("应可解析"),
            vec!["GET", "user:1"]
        );
    }

    #[test]
    fn keeps_quoted_arguments() {
        assert_eq!(
            parse_command("HSET user:1 name \"alice smith\"").expect("应可解析"),
            vec!["HSET", "user:1", "name", "alice smith"]
        );
        assert_eq!(
            parse_command("SET k 'a b'").expect("应可解析"),
            vec!["SET", "k", "a b"]
        );
    }

    #[test]
    fn handles_escape_and_errors() {
        assert_eq!(
            parse_command("SET k \"a\\\"b\"").expect("应可解析"),
            vec!["SET", "k", "a\"b"]
        );
        assert!(parse_command("GET \"unterminated").is_err());
        assert!(parse_command("   ").is_err());
    }
}
