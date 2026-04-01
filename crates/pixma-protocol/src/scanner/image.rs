use std::path::Path;

use crate::error::PixmaError;
use crate::scanner::session::ScanResult;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

impl OutputFormat {
    pub fn from_extension(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("jpg" | "jpeg") => Self::Jpeg,
            _ => Self::Png,
        }
    }
}

pub fn save_scan(
    result: &ScanResult,
    path: &Path,
    format: OutputFormat,
) -> Result<(), PixmaError> {
    if result.is_jpeg {
        match format {
            OutputFormat::Jpeg => {
                // Write raw JPEG bytes directly
                std::fs::write(path, &result.data)
                    .map_err(|e| PixmaError::ScanFailed(e.to_string()))
            }
            OutputFormat::Png => {
                // Decode JPEG, re-encode as PNG
                let img = image::load_from_memory(&result.data)
                    .map_err(|e| PixmaError::ScanFailed(format!("JPEG decode failed: {e}")))?;
                img.save(path)
                    .map_err(|e| PixmaError::ScanFailed(e.to_string()))
            }
        }
    } else {
        // Raw pixel data (not used with CHMP, but kept for future USB support)
        let img = image::load_from_memory(&result.data)
            .map_err(|e| PixmaError::ScanFailed(format!("image decode failed: {e}")))?;
        match format {
            OutputFormat::Png => img.save(path).map_err(|e| PixmaError::ScanFailed(e.to_string())),
            OutputFormat::Jpeg => img
                .save_with_format(path, image::ImageFormat::Jpeg)
                .map_err(|e| PixmaError::ScanFailed(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_extension() {
        assert!(matches!(OutputFormat::from_extension(Path::new("scan.png")), OutputFormat::Png));
        assert!(matches!(OutputFormat::from_extension(Path::new("scan.jpg")), OutputFormat::Jpeg));
        assert!(matches!(OutputFormat::from_extension(Path::new("scan.jpeg")), OutputFormat::Jpeg));
        assert!(matches!(OutputFormat::from_extension(Path::new("scan")), OutputFormat::Png));
    }

    #[test]
    fn save_jpeg_data() {
        // Minimal valid JPEG (1x1 white pixel)
        let jpeg_data = include_bytes!("../../test_data/1x1_white.jpg").to_vec();
        let result = ScanResult {
            data: jpeg_data,
            is_jpeg: true,
            width: 1,
            height: 1,
            channels: 3,
        };

        let dir = std::env::temp_dir();
        let path = dir.join("pixma_test_jpeg.jpg");
        save_scan(&result, &path, OutputFormat::Jpeg).unwrap();
        assert!(path.exists());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_jpeg_as_png() {
        let jpeg_data = include_bytes!("../../test_data/1x1_white.jpg").to_vec();
        let result = ScanResult {
            data: jpeg_data,
            is_jpeg: true,
            width: 1,
            height: 1,
            channels: 3,
        };

        let dir = std::env::temp_dir();
        let path = dir.join("pixma_test_jpeg_to_png.png");
        save_scan(&result, &path, OutputFormat::Png).unwrap();
        assert!(path.exists());
        std::fs::remove_file(&path).ok();
    }
}
