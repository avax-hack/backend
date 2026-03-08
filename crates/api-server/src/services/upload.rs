use openlaunch_shared::error::AppResult;

/// Placeholder upload service.
/// TODO: Integrate with S3/R2 for actual file storage.
/// For now, returns the filename as the URI.
pub async fn upload_image(filename: &str, _data: &[u8]) -> AppResult<String> {
    let uri = format!("/uploads/images/{filename}");
    tracing::info!("Placeholder image upload: {uri}");
    Ok(uri)
}

/// Placeholder evidence upload.
/// TODO: Integrate with S3/R2 for actual file storage.
pub async fn upload_evidence(filename: &str, _data: &[u8]) -> AppResult<String> {
    let uri = format!("/uploads/evidence/{filename}");
    tracing::info!("Placeholder evidence upload: {uri}");
    Ok(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upload_image_returns_correct_uri() {
        let result = upload_image("photo.png", b"fake_image_data").await.unwrap();
        assert_eq!(result, "/uploads/images/photo.png");
    }

    #[tokio::test]
    async fn upload_image_handles_special_characters_in_filename() {
        let result = upload_image("my file (1).jpg", b"data").await.unwrap();
        assert_eq!(result, "/uploads/images/my file (1).jpg");
    }

    #[tokio::test]
    async fn upload_image_empty_data_still_returns_uri() {
        let result = upload_image("empty.png", b"").await.unwrap();
        assert_eq!(result, "/uploads/images/empty.png");
    }

    #[tokio::test]
    async fn upload_evidence_returns_correct_uri() {
        let result = upload_evidence("doc.pdf", b"fake_evidence_data").await.unwrap();
        assert_eq!(result, "/uploads/evidence/doc.pdf");
    }

    #[tokio::test]
    async fn upload_evidence_handles_various_extensions() {
        let result = upload_evidence("proof.zip", b"data").await.unwrap();
        assert_eq!(result, "/uploads/evidence/proof.zip");
    }
}
