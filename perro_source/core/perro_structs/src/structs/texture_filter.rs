#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextureFilterMode {
    Nearest,
    Linear,
    #[default]
    LinearMipmap,
    Anisotropic,
}

impl TextureFilterMode {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Linear => "linear",
            Self::LinearMipmap => "linear_mipmap",
            Self::Anisotropic => "anisotropic",
        }
    }

    #[inline]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "nearest" | "point" | "pixel" | "pixel_art" => Some(Self::Nearest),
            "linear" | "bilinear" | "smooth" => Some(Self::Linear),
            "linear_mipmap" | "linear_mip" | "mipmap" | "trilinear" => Some(Self::LinearMipmap),
            "anisotropic" | "aniso" => Some(Self::Anisotropic),
            _ => None,
        }
    }

    #[inline]
    pub const fn uses_mipmaps(self) -> bool {
        matches!(self, Self::LinearMipmap | Self::Anisotropic)
    }
}
