//! SVG path subset, authored for Forja's stroke sources.
use super::arc;
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Command {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([f64; 2], [f64; 2], [f64; 2]),
    Quadratic([f64; 2], [f64; 2]),
    Close,
}
#[derive(Clone, Copy)]
enum Token {
    Letter(char),
    Number(f64),
}
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    let mut result = Vec::new();
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_whitespace() || ch == ',' {
            index += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() {
            result.push(Token::Letter(ch));
            index += 1;
            continue;
        }
        let start = index;
        if matches!(ch, '+' | '-') {
            index += 1;
        }
        while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        if index < chars.len() && matches!(chars[index], 'e' | 'E') {
            index += 1;
            if index < chars.len() && matches!(chars[index], '+' | '-') {
                index += 1;
            }
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
        }
        if index == start {
            return Err(format!("invalid path character {ch}"));
        }
        let value: f64 = chars[start..index]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| "invalid number")?;
        if !value.is_finite() {
            return Err("non-finite coordinate".into());
        }
        result.push(Token::Number(value));
    }
    Ok(result)
}
pub(super) fn parse(text: &str) -> Result<Vec<Command>, String> {
    let tokens = tokens(text)?;
    let mut index = 0;
    let mut code = None;
    let mut pen = [0.0, 0.0];
    let mut start = pen;
    let mut out = Vec::new();
    while index < tokens.len() {
        if let Token::Letter(ch) = tokens[index] {
            code = Some(ch);
            index += 1;
        }
        let c = code.ok_or("path needs a command")?;
        let upper = c.to_ascii_uppercase();
        let relative = c.is_ascii_lowercase();
        if upper == 'Z' {
            out.push(Command::Close);
            pen = start;
            code = None;
            continue;
        }
        let count = match upper {
            'M' | 'L' => 2,
            'H' | 'V' => 1,
            'C' => 6,
            'Q' => 4,
            'A' => 7,
            _ => return Err(format!("unsupported path command {c}")),
        };
        let mut args = Vec::with_capacity(count);
        for _ in 0..count {
            match tokens.get(index) {
                Some(Token::Number(value)) => {
                    args.push(*value);
                    index += 1;
                }
                _ => return Err(format!("missing argument for {c}")),
            }
        }
        let point = |x: f64, y: f64| {
            if relative {
                [x + pen[0], y + pen[1]]
            } else {
                [x, y]
            }
        };
        match upper {
            'M' => {
                pen = point(args[0], args[1]);
                start = pen;
                out.push(Command::Move(pen));
                code = Some(if relative { 'l' } else { 'L' });
            }
            'L' => {
                pen = point(args[0], args[1]);
                out.push(Command::Line(pen));
            }
            'H' => {
                pen[0] = if relative { pen[0] + args[0] } else { args[0] };
                out.push(Command::Line(pen));
            }
            'V' => {
                pen[1] = if relative { pen[1] + args[0] } else { args[0] };
                out.push(Command::Line(pen));
            }
            'C' => {
                let a = point(args[0], args[1]);
                let b = point(args[2], args[3]);
                let end = point(args[4], args[5]);
                out.push(Command::Cubic(a, b, end));
                pen = end;
            }
            'Q' => {
                let a = point(args[0], args[1]);
                let end = point(args[2], args[3]);
                out.push(Command::Quadratic(a, end));
                pen = end;
            }
            'A' => {
                if !matches!(args[3], 0.0 | 1.0) || !matches!(args[4], 0.0 | 1.0) {
                    return Err("arc flags must be 0 or 1".into());
                }
                let end = point(args[5], args[6]);
                out.extend(arc::convert(
                    pen,
                    end,
                    args[0],
                    args[1],
                    args[2],
                    args[3] != 0.0,
                    args[4] != 0.0,
                ));
                pen = end;
            }
            _ => unreachable!(),
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_relative_and_exponent_coordinates() {
        assert_eq!(
            parse("M3 4 5 6h2v-1 l1e0 2z").unwrap(),
            vec![
                Command::Move([3., 4.]),
                Command::Line([5., 6.]),
                Command::Line([7., 6.]),
                Command::Line([7., 5.]),
                Command::Line([8., 7.]),
                Command::Close
            ]
        );
    }
    #[test]
    fn curves_and_arc_flags_are_validated() {
        assert!(matches!(
            parse("M0 0Q1 2 3 4C4 4 5 5 6 6").unwrap()[1],
            Command::Quadratic(..)
        ));
        assert!(parse("M1 1 A4 4 0 2 0 9 9").is_err());
        assert!(parse("M1").is_err());
        assert!(parse("M0 0T2 2").is_err());
    }
}
