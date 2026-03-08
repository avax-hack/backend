use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Credentials, Region};

use crate::config;

/// S3-compatible client for Cloudflare R2.
#[derive(Clone)]
pub struct R2Client {
    client: Client,
}

impl R2Client {
    /// Initialize the R2 client using config env vars.
    pub async fn new() -> anyhow::Result<Self> {
        let endpoint = format!(
            "https://{}.r2.cloudflarestorage.com",
            *config::R2_ACCOUNT_ID
        );

        let credentials = Credentials::new(
            &*config::R2_ACCESS_KEY_ID,
            &*config::R2_SECRET_ACCESS_KEY,
            None,
            None,
            "r2",
        );

        let config = aws_sdk_s3::Config::builder()
            .behavior_version_latest()
            .endpoint_url(&endpoint)
            .region(Region::new("auto"))
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(config);

        Ok(Self { client })
    }

    /// Upload bytes to a bucket with the given key and content type.
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> anyhow::Result<()> {
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body.into())
            .content_type(content_type)
            .send()
            .await?;

        Ok(())
    }

    /// Build the public URL for an object in the image bucket.
    pub fn image_url(key: &str) -> String {
        format!("{}/{}", *config::R2_IMAGE_PUBLIC_URL, key)
    }

    /// Build the public URL for an object in the metadata bucket.
    pub fn metadata_url(key: &str) -> String {
        format!("{}/{}", *config::R2_METADATA_PUBLIC_URL, key)
    }
}
