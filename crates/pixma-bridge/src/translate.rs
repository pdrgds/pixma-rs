use pixma_protocol::scanner::commands::{ColorMode, ScanParams};
use quick_xml::Reader;
use quick_xml::events::Event;

/// Parsed eSCL scan settings from the POST body.
#[derive(Debug)]
pub struct EsclScanSettings {
    pub x_resolution: u16,
    pub y_resolution: u16,
    pub color_mode: ColorMode,
    pub x_offset: u32,
    pub y_offset: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for EsclScanSettings {
    fn default() -> Self {
        Self {
            x_resolution: 300,
            y_resolution: 300,
            color_mode: ColorMode::Color,
            x_offset: 0,
            y_offset: 0,
            width: 2550,
            height: 3508,
        }
    }
}

impl EsclScanSettings {
    /// Convert eSCL settings to CHMP ScanParams.
    /// eSCL dimensions are in 1/300-inch units; ScanParams needs pixels at the target DPI.
    pub fn to_scan_params(&self) -> ScanParams {
        let scale = self.x_resolution as f64 / 300.0;
        let width = (self.width as f64 * scale) as u32;
        let height = (self.height as f64 * scale) as u32;
        let x = (self.x_offset as f64 * scale) as u32;
        let y = (self.y_offset as f64 * scale) as u32;

        ScanParams {
            dpi: self.x_resolution,
            color: self.color_mode,
            x,
            y,
            width,
            height,
        }
    }
}

/// Parse eSCL ScanSettings XML from a POST body.
pub fn parse_scan_settings(xml: &[u8]) -> EsclScanSettings {
    let mut settings = EsclScanSettings::default();
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut current_tag = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                current_tag = name;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "XResolution" => {
                        if let Ok(v) = text.parse() {
                            settings.x_resolution = v;
                        }
                    }
                    "YResolution" => {
                        if let Ok(v) = text.parse() {
                            settings.y_resolution = v;
                        }
                    }
                    "ColorMode" => {
                        settings.color_mode = match text.as_str() {
                            "Grayscale8" => ColorMode::Grayscale,
                            _ => ColorMode::Color,
                        };
                    }
                    "XOffset" => {
                        if let Ok(v) = text.parse() {
                            settings.x_offset = v;
                        }
                    }
                    "YOffset" => {
                        if let Ok(v) = text.parse() {
                            settings.y_offset = v;
                        }
                    }
                    "Width" => {
                        if let Ok(v) = text.parse() {
                            settings.width = v;
                        }
                    }
                    "Height" => {
                        if let Ok(v) = text.parse() {
                            settings.height = v;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_REQUEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScanSettings xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm"
                   xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03">
    <pwg:Version>2.0</pwg:Version>
    <pwg:ScanRegions>
        <pwg:ScanRegion>
            <pwg:ContentRegionUnits>escl:ThreeHundredthsOfInches</pwg:ContentRegionUnits>
            <pwg:XOffset>0</pwg:XOffset>
            <pwg:YOffset>0</pwg:YOffset>
            <pwg:Width>2550</pwg:Width>
            <pwg:Height>3300</pwg:Height>
        </pwg:ScanRegion>
    </pwg:ScanRegions>
    <pwg:InputSource>Platen</pwg:InputSource>
    <scan:ColorMode>RGB24</scan:ColorMode>
    <scan:XResolution>300</scan:XResolution>
    <scan:YResolution>300</scan:YResolution>
    <pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat>
</scan:ScanSettings>"#;

    #[test]
    fn parse_300dpi_color() {
        let settings = parse_scan_settings(SAMPLE_REQUEST.as_bytes());
        assert_eq!(settings.x_resolution, 300);
        assert_eq!(settings.color_mode, ColorMode::Color);
        assert_eq!(settings.width, 2550);
        assert_eq!(settings.height, 3300);
    }

    #[test]
    fn parse_grayscale() {
        let xml = SAMPLE_REQUEST.replace("RGB24", "Grayscale8");
        let settings = parse_scan_settings(xml.as_bytes());
        assert_eq!(settings.color_mode, ColorMode::Grayscale);
    }

    #[test]
    fn to_scan_params_300dpi() {
        let settings = EsclScanSettings {
            x_resolution: 300,
            y_resolution: 300,
            color_mode: ColorMode::Color,
            x_offset: 0,
            y_offset: 0,
            width: 2550,
            height: 3300,
        };
        let params = settings.to_scan_params();
        assert_eq!(params.dpi, 300);
        assert_eq!(params.width, 2550);
        assert_eq!(params.height, 3300);
    }

    #[test]
    fn to_scan_params_600dpi_scales() {
        let settings = EsclScanSettings {
            x_resolution: 600,
            y_resolution: 600,
            color_mode: ColorMode::Color,
            x_offset: 0,
            y_offset: 0,
            width: 2550,
            height: 3300,
        };
        let params = settings.to_scan_params();
        assert_eq!(params.dpi, 600);
        assert_eq!(params.width, 5100); // 2550 * 2
        assert_eq!(params.height, 6600); // 3300 * 2
    }

    #[test]
    fn defaults_on_empty_xml() {
        let settings = parse_scan_settings(b"<scan:ScanSettings/>");
        assert_eq!(settings.x_resolution, 300);
        assert_eq!(settings.color_mode, ColorMode::Color);
    }
}
