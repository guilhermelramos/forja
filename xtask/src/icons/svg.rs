//! Strict, dependency-free reader for the small SVG subset used by Forja.
use super::path::{self, Command};
use std::collections::BTreeMap;
fn attributes(mut text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    loop {
        text = text.trim_start();
        if text.is_empty() || text == "/" {
            break;
        }
        let end = text.find('=').ok_or("missing attribute equals")?;
        let key = text[..end].trim();
        text = text[end + 1..].trim_start();
        let quote = text.chars().next().ok_or("missing quote")?;
        if quote != '\'' && quote != '"' {
            return Err("quoted attribute required".into());
        }
        text = &text[1..];
        let end = text.find(quote).ok_or("unclosed attribute")?;
        if out.insert(key.to_owned(), text[..end].to_owned()).is_some() {
            return Err("duplicate attribute".into());
        }
        text = &text[end + 1..];
    }
    Ok(out)
}
pub(super) fn parse(text: &str) -> Result<Vec<Command>, String> {
    let mut commands = Vec::new();
    let mut rest = text;
    let mut root = false;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if rest.starts_with("!--") {
            let end = rest.find("-->").ok_or("unclosed comment")?;
            rest = &rest[end + 3..];
            continue;
        }
        let end = rest.find('>').ok_or("unclosed tag")?;
        let tag = rest[..end].trim();
        rest = &rest[end + 1..];
        if tag.starts_with('/') {
            continue;
        }
        let split = tag.find(char::is_whitespace).unwrap_or(tag.len());
        let name = &tag[..split];
        let attrs = attributes(&tag[split..])?;
        let allowed: &[&str] = match name {
            "svg" => &[
                "xmlns",
                "viewBox",
                "fill",
                "stroke",
                "stroke-width",
                "stroke-linecap",
                "stroke-linejoin",
            ],
            "path" => &["d"],
            "line" => &["x1", "y1", "x2", "y2"],
            "polyline" => &["points"],
            "circle" => &["cx", "cy", "r"],
            "rect" => &["x", "y", "width", "height", "rx", "ry"],
            _ => return Err(format!("unsupported SVG element {name}")),
        };
        if let Some(key) = attrs.keys().find(|key| !allowed.contains(&key.as_str())) {
            return Err(format!("unsupported {name} attribute {key}"));
        }
        let number = |key: &str, default: Option<f64>| -> Result<f64, String> {
            let value = match attrs.get(key) {
                Some(value) => value.parse::<f64>().map_err(|_| format!("invalid {key}"))?,
                None => default.ok_or_else(|| format!("missing {key}"))?,
            };
            if value.is_finite() {
                Ok(value)
            } else {
                Err("non-finite SVG coordinate".into())
            }
        };
        match name {
            "svg" => {
                root = true;
                if attrs.get("viewBox").map(String::as_str) != Some("0 0 24 24")
                    || attrs.get("fill").map(String::as_str) != Some("none")
                    || number("stroke-width", None)? != 2.0
                    || attrs.get("stroke-linecap").map(String::as_str) != Some("round")
                    || attrs.get("stroke-linejoin").map(String::as_str) != Some("round")
                {
                    return Err("icons require the 24px stroke contract".into());
                }
            }
            "path" => commands.extend(path::parse(attrs.get("d").ok_or("missing d")?)?),
            "line" => {
                commands.push(Command::Move([
                    number("x1", Some(0.))?,
                    number("y1", Some(0.))?,
                ]));
                commands.push(Command::Line([
                    number("x2", Some(0.))?,
                    number("y2", Some(0.))?,
                ]));
            }
            "polyline" => {
                let data = attrs.get("points").ok_or("missing points")?;
                commands.extend(path::parse(&format!("M{data}"))?);
            }
            "circle" => {
                let (cx, cy, r) = (
                    number("cx", Some(0.))?,
                    number("cy", Some(0.))?,
                    number("r", None)?,
                );
                if r <= 0. {
                    return Err("positive circle radius required".into());
                }
                commands.extend(path::parse(&format!(
                    "M{} {} A{r} {r} 0 0 1 {} {} A{r} {r} 0 0 1 {} {} Z",
                    cx - r,
                    cy,
                    cx + r,
                    cy,
                    cx - r,
                    cy
                ))?);
            }
            "rect" => {
                let (x, y, w, h) = (
                    number("x", Some(0.))?,
                    number("y", Some(0.))?,
                    number("width", None)?,
                    number("height", None)?,
                );
                if w <= 0. || h <= 0. {
                    return Err("positive rectangle dimensions required".into());
                }
                let rx = number("rx", Some(0.))?.min(w / 2.).max(0.);
                let ry = number("ry", Some(rx))?.min(h / 2.).max(0.);
                let d = if rx == 0. || ry == 0. {
                    format!("M{x} {y}h{w}v{h}h{}Z", -w)
                } else {
                    format!("M{} {y}H{}Q{} {y} {} {}V{}Q{} {} {} {}H{}Q{x} {} {x} {}V{}Q{x} {y} {} {y}Z",x+rx,x+w-rx,x+w,x+w,y+ry,y+h-ry,x+w,y+h,x+w-rx,y+h,x+rx,y+h,y+h-ry,y+ry,x+rx)
                };
                commands.extend(path::parse(&d)?);
            }
            _ => return Err(format!("unsupported SVG element {name}")),
        }
    }
    if !root || commands.is_empty() {
        return Err("empty icon".into());
    }
    Ok(commands)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_shape_overrides_that_the_stroke_table_cannot_represent() {
        let root = "<svg viewBox='0 0 24 24' fill='none' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'>";
        for attribute in [
            "fill='red'",
            "stroke-width='5'",
            "stroke-linecap='square'",
            "opacity='0.2'",
        ] {
            let svg = format!("{root}<path d='M3 3L9 9' {attribute}/></svg>");
            assert!(
                parse(&svg).is_err(),
                "ignored appearance override: {attribute}"
            );
        }
    }
    #[test]
    fn shapes_share_the_stroke_contract() {
        let svg="<svg viewBox='0 0 24 24' fill='none' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='6'/><rect x='3' y='4' width='12' height='15' rx='2'/><line x1='1' y1='2' x2='3' y2='4'/><polyline points='2,3 4,5'/></svg>";
        assert!(parse(svg).unwrap().len() > 10);
        assert!(parse(&svg.replace("stroke-width='2'", "stroke-width='3'")).is_err());
        assert!(parse(&svg.replace("circle", "polygon")).is_err());
    }
}
