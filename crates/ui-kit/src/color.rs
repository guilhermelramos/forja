//! Cor sRGB e contraste WCAG 2.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Cor sRGB com alfa. Nos arquivos TOML entra como `#RRGGBB` ou `#RRGGBBAA`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    /// Canal vermelho, 0–255.
    pub r: u8,
    /// Canal verde, 0–255.
    pub g: u8,
    /// Canal azul, 0–255.
    pub b: u8,
    /// Alfa, 0–255. `255` é opaco.
    pub a: u8,
}

impl Color {
    /// Cor opaca a partir dos três canais.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Lê `#RGB`, `#RRGGBB` ou `#RRGGBBAA`.
    pub fn parse(input: &str) -> Result<Self, String> {
        let hex = input.trim().trim_start_matches('#');
        if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(format!("cor inválida: {input}"));
        }
        let expanded;
        let digits = match hex.len() {
            3 => {
                expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
                expanded.as_str()
            }
            6 | 8 => hex,
            _ => return Err(format!("cor inválida: {input}")),
        };
        let value =
            u32::from_str_radix(digits, 16).map_err(|_| format!("cor inválida: {input}"))?;
        if digits.len() == 6 {
            Ok(Self::rgb(byte(value >> 16), byte(value >> 8), byte(value)))
        } else {
            Ok(Self {
                r: byte(value >> 24),
                g: byte(value >> 16),
                b: byte(value >> 8),
                a: byte(value),
            })
        }
    }

    /// `#RRGGBB`, ou `#RRGGBBAA` quando o alfa não é opaco.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    pub fn to_color32(self) -> eframe::egui::Color32 {
        eframe::egui::Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }
}

fn byte(value: u32) -> u8 {
    (value & 0xFF) as u8
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Color::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Luminância relativa WCAG (sRGB linearizado).
pub fn luminance(color: Color) -> f32 {
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

fn channel(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Razão de contraste entre duas cores, de 1 a 21.
pub fn contrast_ratio(left: Color, right: Color) -> f32 {
    let a = luminance(left);
    let b = luminance(right);
    let (light, dark) = if a > b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let color = Color::parse("#F2762E").unwrap();
        assert_eq!(color, Color::rgb(0xF2, 0x76, 0x2E));
        assert_eq!(color.to_hex(), "#F2762E");
    }
}
