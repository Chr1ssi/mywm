use crate::floating::Rect;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Color(pub [u32; 4]);

impl Color {
    pub fn css(self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            self.0[0] / 0x01010101,
            self.0[1] / 0x01010101,
            self.0[2] / 0x01010101
        )
    }
}

impl TryFrom<String> for Color {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value
            .strip_prefix('#')
            .filter(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| "border colors must use #RRGGBB".to_string())?;
        let mut channels = [u32::MAX; 4];
        for (index, channel) in channels[..3].iter_mut().enumerate() {
            *channel =
                u32::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap() * 0x01010101;
        }
        Ok(Self(channels))
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub gaps_inner: i32,
    pub gaps_outer: i32,
    pub border_width: i32,
    pub active_border: Color,
    pub inactive_border: Color,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub muted_text: Color,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            background: Color::try_from("#1e1e2e".to_string()).unwrap(),
            surface: Color::try_from("#313244".to_string()).unwrap(),
            text: Color::try_from("#cdd6f4".to_string()).unwrap(),
            muted_text: Color::try_from("#a6adc8".to_string()).unwrap(),
            gaps_inner: 8,
            gaps_outer: 8,
            border_width: 2,
            active_border: Color::try_from("#89b4fa".to_string()).unwrap(),
            inactive_border: Color::try_from("#45475a".to_string()).unwrap(),
        }
    }
}

impl Appearance {
    pub fn validate(&self) -> Result<(), String> {
        if !(0..=128).contains(&self.gaps_inner) || !(0..=128).contains(&self.gaps_outer) {
            return Err("appearance gaps must be between 0 and 128".into());
        }
        if !(0..=32).contains(&self.border_width) {
            return Err("appearance.border_width must be between 0 and 32".into());
        }
        Ok(())
    }

    pub fn viewport(&self, area: Rect) -> Rect {
        let gap = self
            .gaps_outer
            .min((area.width - 1).max(0) / 2)
            .min((area.height - 1).max(0) / 2);
        Rect {
            x: area.x + gap,
            y: area.y + gap,
            width: area.width - 2 * gap,
            height: area.height - 2 * gap,
        }
    }

    /// River positions/sizes refer to content, with borders outside that content.
    pub fn content(&self, frame: Rect) -> (Rect, i32) {
        let border = self
            .border_width
            .min((frame.width - 1).max(0) / 2)
            .min((frame.height - 1).max(0) / 2);
        (
            Rect {
                x: frame.x + border,
                y: frame.y + border,
                width: frame.width - 2 * border,
                height: frame.height - 2 * border,
            },
            border,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    #[test]
    fn borders_and_gaps_fit_inside_work_area() {
        let style = Appearance::default();
        let area = Rect {
            x: 100,
            y: 240,
            width: 1920,
            height: 1000,
        };
        let viewport = style.viewport(area);
        let (content, border) = style.content(viewport);
        assert_eq!(
            content,
            Rect {
                x: 110,
                y: 250,
                width: 1900,
                height: 980
            }
        );
        assert_eq!(border, 2);
        assert_eq!(
            style.content(style.viewport(Rect {
                width: 1,
                height: 1,
                ..area
            })),
            (
                Rect {
                    width: 1,
                    height: 1,
                    ..area
                },
                0
            )
        );
    }
    #[test]
    fn validates_colors_and_sizes() {
        assert_eq!(
            Color::try_from("#ff8000".to_string()).unwrap().0,
            [u32::MAX, 0x80808080, 0, u32::MAX]
        );
        for setting in [
            "gaps_inner = -1",
            "gaps_outer = 129",
            "border_width = 33",
            "active_border = 'blue'",
            "inactive_border = '#12345g'",
            "active_border = '#ffffffff'",
        ] {
            assert!(Config::parse(&format!("[appearance]\n{setting}")).is_err());
        }
        let config =
            Config::parse("[appearance]\ngaps_inner = 0\ngaps_outer = 0\nborder_width = 0")
                .unwrap();
        let area = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            config.appearance.content(config.appearance.viewport(area)),
            (area, 0)
        );
    }
}
