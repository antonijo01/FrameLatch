/// Persisted indices 0–3 and the worker's `small`/`standard` protocol are legacy-compatible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Standard,
    Small,
    Portrait,
    PortraitSmall,
    High,
    Ultra,
    Original,
}
impl Quality {
    pub fn from_index(index: usize) -> Option<Self> {
        Some(match index {
            0 => Self::Standard,
            1 => Self::Small,
            2 => Self::Portrait,
            3 => Self::PortraitSmall,
            4 => Self::High,
            5 => Self::Ultra,
            6 => Self::Original,
            _ => return None,
        })
    }
    pub fn from_worker(name: &str, vertical: bool) -> Option<Self> {
        match (name, vertical) {
            ("standard", false) => Some(Self::Standard),
            ("small", false) => Some(Self::Small),
            ("standard", true) => Some(Self::Portrait),
            ("small", true) => Some(Self::PortraitSmall),
            ("high", false) => Some(Self::High),
            ("ultra", false) => Some(Self::Ultra),
            ("original", false) => Some(Self::Original),
            _ => None,
        }
    }
    pub fn worker_args(self) -> (&'static str, &'static str) {
        match self {
            Self::Standard => ("standard", "landscape"),
            Self::Small => ("small", "landscape"),
            Self::Portrait => ("standard", "vertical"),
            Self::PortraitSmall => ("small", "vertical"),
            Self::High => ("high", "landscape"),
            Self::Ultra => ("ultra", "landscape"),
            Self::Original => ("original", "landscape"),
        }
    }
    pub fn summary(self) -> &'static str {
        match self {
            Self::Standard => "Up to 1080p",
            Self::Small => "Up to 720p",
            Self::Portrait => "1080p · Portrait",
            Self::PortraitSmall => "720p · Portrait",
            Self::High => "Up to 1440p",
            Self::Ultra => "Up to 4K",
            Self::Original => "Original · Native size",
        }
    }
    pub fn dimensions(self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width < 2 || height < 2 {
            return None;
        }
        match self {
            Self::Portrait => Some((1080, 1920)),
            Self::PortraitSmall => Some((720, 1280)),
            other => {
                let limits = match other {
                    Self::Standard => Some((1920, 1080)),
                    Self::Small => Some((1280, 720)),
                    Self::High => Some((2560, 1440)),
                    Self::Ultra => Some((3840, 2160)),
                    Self::Original => None,
                    _ => unreachable!(),
                };
                let scale = limits.map_or(1.0, |(max_w, max_h)| {
                    (f64::from(max_w) / f64::from(width))
                        .min(f64::from(max_h) / f64::from(height))
                        .min(1.0)
                });
                let w = ((f64::from(width) * scale) as u32) & !1;
                let h = ((f64::from(height) * scale) as u32) & !1;
                (w >= 2 && h >= 2).then_some((w, h))
            }
        }
    }
    pub fn bitrate(self, width: u32, height: u32) -> u32 {
        match self {
            Self::Small | Self::PortraitSmall => 3_000_000,
            Self::Standard | Self::Portrait => 8_000_000,
            Self::High | Self::Ultra | Self::Original => {
                let pixels = u64::from(width) * u64::from(height);
                (pixels.saturating_mul(8_000_000) / 2_073_600).clamp(8_000_000, 80_000_000) as u32
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::Quality;
    #[test]
    fn indices_and_worker_protocol_round_trip() {
        let expected = [
            Quality::Standard,
            Quality::Small,
            Quality::Portrait,
            Quality::PortraitSmall,
            Quality::High,
            Quality::Ultra,
            Quality::Original,
        ];
        for (i, preset) in expected.iter().copied().enumerate() {
            assert_eq!(Quality::from_index(i), Some(preset));
            let (name, orientation) = preset.worker_args();
            assert_eq!(
                Quality::from_worker(name, orientation == "vertical"),
                Some(preset)
            );
        }
        assert_eq!(Quality::from_index(7), None);
        assert_eq!(Quality::from_worker("original", true), None);
    }
    #[test]
    fn high_quality_limits_and_native_odd_dimensions() {
        assert_eq!(Quality::High.dimensions(3000, 1876), Some((2302, 1440)));
        assert_eq!(Quality::High.dimensions(3840, 2160), Some((2560, 1440)));
        assert_eq!(Quality::Ultra.dimensions(3840, 2160), Some((3840, 2160)));
        assert_eq!(Quality::Original.dimensions(3001, 1877), Some((3000, 1876)));
        for preset in [Quality::High, Quality::Ultra, Quality::Original] {
            assert_eq!(preset.dimensions(801, 501), Some((800, 500)));
        }
        assert_eq!(Quality::Standard.dimensions(3840, 2160), Some((1920, 1080)));
        assert_eq!(Quality::Small.dimensions(3000, 1876), Some((1150, 720)));
        assert_eq!(Quality::Portrait.dimensions(800, 500), Some((1080, 1920)));
        assert_eq!(
            Quality::PortraitSmall.dimensions(800, 500),
            Some((720, 1280))
        );
        assert_eq!(Quality::High.dimensions(1, 10), None);
    }
    #[test]
    fn large_frame_bitrate_scales_with_actual_output_pixels() {
        assert_eq!(Quality::Ultra.bitrate(3840, 2160), 32_000_000);
        assert_eq!(Quality::High.bitrate(800, 500), 8_000_000);
        assert_eq!(Quality::Standard.bitrate(1920, 1080), 8_000_000);
        assert_eq!(Quality::PortraitSmall.bitrate(720, 1280), 3_000_000);
    }
}
