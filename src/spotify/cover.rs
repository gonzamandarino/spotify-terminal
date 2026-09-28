//! Tapa del disco del tema que suena, para el vinilo (spec 010).
//! Spotify las sirve en JPEG (`image` con `jpeg`); PNG también se lee.

use image::{GenericImageView, imageops::FilterType};

use crate::{config, error::AppError};

/// Imagen RGBA lista para dibujar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub width: u32,
    pub height: u32,
    /// `width × height × 4` bytes, fila por fila.
    pub rgba: Vec<u8>,
}

/// Descarga y decodifica la tapa de `url` (una imagen de `i.scdn.co`).
///
/// - Post: a lo sumo `COVER_SIZE` px de lado (se achica si viene más
///   grande); `rgba.len() == width × height × 4`.
/// - Errores: `Network` si no se pudo bajar o pasa `COVER_MAX_BYTES`;
///   `Internal` si no es una imagen JPEG/PNG válida.
/// - No debe: decodificar en el hilo que la llama (corre en uno de
///   bloqueo: no traba al motor).
pub async fn fetch(url: &str) -> Result<Cover, AppError> {
    let client = reqwest::Client::builder()
        .timeout(config::HTTP_TIMEOUT)
        .build()
        .map_err(|e| AppError::Network(format!("tapa: {e}")))?;
    let response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| AppError::Network(format!("tapa: {e}")))?;
    if response
        .content_length()
        .is_some_and(|len| len > config::viz::COVER_MAX_BYTES as u64)
    {
        return Err(AppError::Network("tapa demasiado grande".into()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Network(format!("tapa: {e}")))?;
    if bytes.len() > config::viz::COVER_MAX_BYTES {
        return Err(AppError::Network("tapa demasiado grande".into()));
    }
    tokio::task::spawn_blocking(move || decode(&bytes))
        .await
        .map_err(|e| AppError::Internal(format!("tapa: {e}")))?
}

/// `bytes` (JPEG o PNG) a RGBA, achicada a `COVER_SIZE` si hace falta.
fn decode(bytes: &[u8]) -> Result<Cover, AppError> {
    let mut image =
        image::load_from_memory(bytes).map_err(|e| AppError::Internal(format!("tapa: {e}")))?;
    let max = config::viz::COVER_SIZE;
    if image.width() > max || image.height() > max {
        image = image.resize(max, max, FilterType::Triangle);
    }
    let (width, height) = image.dimensions();
    Ok(Cover {
        width,
        height,
        rgba: image.into_rgba8().into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las tapas de Spotify son JPEG.
    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb([200, 40, 30]));
        let mut out = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .expect("JPEG en memoria");
        out.into_inner()
    }

    #[test]
    fn decodifica_y_achica_si_viene_grande() {
        let small = decode(&jpeg(64, 64)).unwrap();
        assert_eq!((small.width, small.height), (64, 64));
        assert_eq!(small.rgba.len(), 64 * 64 * 4);
        let [r, g, b, a] = [0, 1, 2, 3].map(|i| i32::from(small.rgba[i]));
        assert!((r - 200).abs() < 8 && (g - 40).abs() < 8 && (b - 30).abs() < 8);
        assert_eq!(a, 255);
        let big = decode(&jpeg(640, 640)).unwrap();
        assert_eq!(
            (big.width, big.height),
            (config::viz::COVER_SIZE, config::viz::COVER_SIZE)
        );
    }

    #[test]
    fn basura_da_error_sin_panic() {
        assert!(decode(b"no es una imagen").is_err());
    }
}
