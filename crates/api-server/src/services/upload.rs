use openlaunch_shared::config;
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::storage::r2::R2Client;

const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024; // 5MB

const ALLOWED_IMAGE_TYPES: &[(&str, &[u8], &str)] = &[
    ("image/png", &[0x89, 0x50, 0x4E, 0x47], "png"),
    ("image/jpeg", &[0xFF, 0xD8, 0xFF], "jpg"),
    ("image/webp", &[0x52, 0x49, 0x46, 0x46], "webp"),
    ("image/gif", &[0x47, 0x49, 0x46], "gif"),
];

/// Detect image type by magic bytes. Returns (content_type, extension).
pub fn detect_image_type(data: &[u8]) -> Option<(&'static str, &'static str)> {
    for &(content_type, magic, ext) in ALLOWED_IMAGE_TYPES {
        if data.len() >= magic.len() && data[..magic.len()] == *magic {
            return Some((content_type, ext));
        }
    }
    None
}

/// Validate that an image_uri points to our R2 image bucket.
pub fn validate_image_uri(uri: &str) -> bool {
    let prefix = &*config::R2_IMAGE_PUBLIC_URL;
    !prefix.is_empty() && uri.starts_with(prefix)
}

/// Upload an image to R2. Returns the public URL.
pub async fn upload_image(r2: &R2Client, data: &[u8]) -> AppResult<String> {
    if data.is_empty() {
        return Err(AppError::BadRequest("Empty file".to_string()));
    }
    if data.len() > MAX_IMAGE_SIZE {
        return Err(AppError::BadRequest(format!(
            "File too large. Max size is {}MB",
            MAX_IMAGE_SIZE / 1024 / 1024
        )));
    }

    let (content_type, ext) = detect_image_type(data)
        .ok_or_else(|| AppError::BadRequest(
            "Invalid image type. Allowed: PNG, JPEG, WebP, GIF".to_string()
        ))?;

    let key = format!("{}.{}", uuid::Uuid::new_v4(), ext);

    r2.put_object(&config::R2_IMAGE_BUCKET, &key, data.to_vec(), content_type)
        .await
        .map_err(AppError::Internal)?;

    Ok(R2Client::image_url(&key))
}

/// Upload evidence file to R2. Returns the public URL.
pub async fn upload_evidence(r2: &R2Client, filename: &str, data: &[u8]) -> AppResult<String> {
    if data.is_empty() {
        return Err(AppError::BadRequest("Empty file".to_string()));
    }
    if data.len() > MAX_IMAGE_SIZE {
        return Err(AppError::BadRequest("File too large".to_string()));
    }

    let ext = filename.rsplit('.').next().unwrap_or("bin");
    let key = format!("{}.{}", uuid::Uuid::new_v4(), ext);

    r2.put_object(&config::R2_IMAGE_BUCKET, &key, data.to_vec(), "application/octet-stream")
        .await
        .map_err(AppError::Internal)?;

    Ok(R2Client::image_url(&key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_png() {
        let (ct, ext) = detect_image_type(&[0x89, 0x50, 0x4E, 0x47, 0x00]).unwrap();
        assert_eq!(ct, "image/png");
        assert_eq!(ext, "png");
    }

    #[test]
    fn detect_jpeg() {
        let (ct, ext) = detect_image_type(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        assert_eq!(ct, "image/jpeg");
        assert_eq!(ext, "jpg");
    }

    #[test]
    fn detect_gif() {
        let (ct, ext) = detect_image_type(&[0x47, 0x49, 0x46, 0x38]).unwrap();
        assert_eq!(ct, "image/gif");
        assert_eq!(ext, "gif");
    }

    #[test]
    fn detect_webp() {
        let (ct, ext) = detect_image_type(&[0x52, 0x49, 0x46, 0x46, 0x00]).unwrap();
        assert_eq!(ct, "image/webp");
        assert_eq!(ext, "webp");
    }

    #[test]
    fn detect_unknown_returns_none() {
        assert!(detect_image_type(&[0x00, 0x00, 0x00]).is_none());
    }

    #[test]
    fn detect_empty_returns_none() {
        assert!(detect_image_type(&[]).is_none());
    }
}
