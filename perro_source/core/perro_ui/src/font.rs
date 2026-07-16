use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiSystemFont {
    #[default]
    SansSerif,
    Serif,
    Monospace,
    Arial,
    Calibri,
    Cambria,
    Consolas,
    CourierNew,
    Georgia,
    Helvetica,
    SegoeUi,
    TimesNewRoman,
    Verdana,
}

impl UiSystemFont {
    pub const fn family_name(self) -> Option<&'static str> {
        match self {
            Self::SansSerif | Self::Serif | Self::Monospace => None,
            Self::Arial => Some("Arial"),
            Self::Calibri => Some("Calibri"),
            Self::Cambria => Some("Cambria"),
            Self::Consolas => Some("Consolas"),
            Self::CourierNew => Some("Courier New"),
            Self::Georgia => Some("Georgia"),
            Self::Helvetica => Some("Helvetica"),
            Self::SegoeUi => Some("Segoe UI"),
            Self::TimesNewRoman => Some("Times New Roman"),
            Self::Verdana => Some("Verdana"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiFont {
    #[default]
    Default,
    System(UiSystemFont),
    Resource(Cow<'static, str>),
}

impl UiFont {
    pub const fn system(font: UiSystemFont) -> Self {
        Self::System(font)
    }

    pub const fn resource(path: &'static str) -> Self {
        Self::Resource(Cow::Borrowed(path))
    }

    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("default") {
            return Some(Self::Default);
        }
        if value.starts_with("res://") || value.starts_with("dlc://") {
            return Some(Self::Resource(Cow::Owned(value.to_string())));
        }
        let name = value.strip_prefix("system://").unwrap_or(value);
        let key = name.to_ascii_lowercase().replace([' ', '-', '_'], "");
        let font = match key.as_str() {
            "sans" | "sansserif" | "systemui" => UiSystemFont::SansSerif,
            "serif" => UiSystemFont::Serif,
            "mono" | "monospace" => UiSystemFont::Monospace,
            "arial" => UiSystemFont::Arial,
            "calibri" => UiSystemFont::Calibri,
            "cambria" => UiSystemFont::Cambria,
            "consolas" => UiSystemFont::Consolas,
            "couriernew" => UiSystemFont::CourierNew,
            "georgia" => UiSystemFont::Georgia,
            "helvetica" => UiSystemFont::Helvetica,
            "segoeui" => UiSystemFont::SegoeUi,
            "timesnewroman" => UiSystemFont::TimesNewRoman,
            "verdana" => UiSystemFont::Verdana,
            _ => return None,
        };
        Some(Self::System(font))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_resource_and_system_fonts() {
        assert_eq!(
            UiFont::parse("system://Segoe UI"),
            Some(UiFont::System(UiSystemFont::SegoeUi))
        );
        assert_eq!(
            UiFont::parse("Arial"),
            Some(UiFont::System(UiSystemFont::Arial))
        );
        assert_eq!(
            UiFont::parse("res://fonts/game.ttf"),
            Some(UiFont::Resource(Cow::Borrowed("res://fonts/game.ttf")))
        );
        assert_eq!(UiFont::parse("missing"), None);
    }
}
