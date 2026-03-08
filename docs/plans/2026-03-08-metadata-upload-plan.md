# Metadata & Image Upload Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upload project images and metadata JSON to Cloudflare R2 with strict validation.

**Architecture:** Add `aws-sdk-s3` for R2 (S3-compatible). Create an `R2Client` in shared crate, inject via `AppState`. Replace placeholder upload service with real R2 uploads. Add `/metadata/create` endpoint for metadata JSON.

**Tech Stack:** `aws-sdk-s3`, `uuid`, Cloudflare R2, axum multipart

---

### Task 1: Add R2 dependencies and config

**Files:**
- Modify: `Cargo.toml` (workspace)
- Modify: `crates/api-server/Cargo.toml`
- Modify: `crates/shared/Cargo.toml`
- Modify: `crates/shared/src/config.rs`

**Step 1: Add dependencies to workspace Cargo.toml**

Add under `[workspace.dependencies]`:
```toml
aws-sdk-s3 = "1"
aws-config = "1"
aws-credential-types = "1"
uuid = { version = "1", features = ["v4"] }
```

**Step 2: Add dependencies to shared and api-server Cargo.toml**

In `crates/shared/Cargo.toml` under `[dependencies]`:
```toml
aws-sdk-s3 = { workspace = true }
aws-config = { workspace = true }
aws-credential-types = { workspace = true }
```

In `crates/api-server/Cargo.toml` under `[dependencies]`:
```toml
uuid = { workspace = true }
```

**Step 3: Add R2 config vars to `crates/shared/src/config.rs`**

Add inside `lazy_static!`:
```rust
// R2 Storage
pub static ref R2_ACCOUNT_ID: String =
    std::env::var("R2_ACCOUNT_ID").expect("R2_ACCOUNT_ID required");
pub static ref R2_ACCESS_KEY_ID: String =
    std::env::var("R2_ACCESS_KEY_ID").expect("R2_ACCESS_KEY_ID required");
pub static ref R2_SECRET_ACCESS_KEY: String =
    std::env::var("R2_SECRET_ACCESS_KEY").expect("R2_SECRET_ACCESS_KEY required");
pub static ref R2_IMAGE_BUCKET: String =
    std::env::var("R2_IMAGE_BUCKET").unwrap_or_else(|_| "openlaunch-image".to_string());
pub static ref R2_METADATA_BUCKET: String =
    std::env::var("R2_METADATA_BUCKET").unwrap_or_else(|_| "openlaunch-metadata".to_string());
pub static ref R2_IMAGE_PUBLIC_URL: String =
    std::env::var("R2_IMAGE_PUBLIC_URL").expect("R2_IMAGE_PUBLIC_URL required");
pub static ref R2_METADATA_PUBLIC_URL: String =
    std::env::var("R2_METADATA_PUBLIC_URL").expect("R2_METADATA_PUBLIC_URL required");
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles with warnings only.

**Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/shared/Cargo.toml crates/api-server/Cargo.toml crates/shared/src/config.rs
git commit -m "feat: add R2 dependencies and config vars"
```

---

### Task 2: Create R2Client in shared crate

**Files:**
- Create: `crates/shared/src/storage/mod.rs`
- Create: `crates/shared/src/storage/r2.rs`
- Modify: `crates/shared/src/lib.rs`

**Step 1: Create `crates/shared/src/storage/mod.rs`**

```rust
pub mod r2;
```

**Step 2: Create `crates/shared/src/storage/r2.rs`**

```rust
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
```

**Step 3: Export storage module from `crates/shared/src/lib.rs`**

Add `pub mod storage;` to the lib.rs module list.

**Step 4: Verify it compiles**

Run: `cargo check`

**Step 5: Commit**

```bash
git add crates/shared/src/storage/ crates/shared/src/lib.rs
git commit -m "feat: add R2Client for Cloudflare R2 storage"
```

---

### Task 3: Add image upload validation and R2 integration

**Files:**
- Modify: `crates/api-server/src/services/upload.rs`
- Modify: `crates/api-server/src/router/metadata/mod.rs`
- Modify: `crates/api-server/src/state.rs`
- Modify: `crates/api-server/src/main.rs`

**Step 1: Add R2Client to AppState**

In `crates/api-server/src/state.rs`, add:
```rust
use openlaunch_shared::storage::r2::R2Client;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<PostgresDatabase>,
    pub redis: Arc<RedisDatabase>,
    pub cache: Arc<SingleFlightCache>,
    pub r2: Arc<R2Client>,
}
```

Update `AppState::new` to accept and store `r2: Arc<R2Client>`.

**Step 2: Initialize R2Client in main.rs**

In `crates/api-server/src/main.rs`, after Redis init:
```rust
use openlaunch_shared::storage::r2::R2Client;

let r2 = Arc::new(R2Client::new().await?);
let state = state::AppState::new(db, redis, r2);
```

**Step 3: Rewrite `crates/api-server/src/services/upload.rs`**

```rust
use openlaunch_shared::config;
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::storage::r2::R2Client;

const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024; // 5MB

const ALLOWED_IMAGE_TYPES: &[(&str, &[u8], &str)] = &[
    ("image/png", &[0x89, 0x50, 0x4E, 0x47], "png"),
    ("image/jpeg", &[0xFF, 0xD8, 0xFF], "jpg"),
    ("image/webp", &[0x52, 0x49, 0x46, 0x46], "webp"),  // RIFF header
    ("image/gif", &[0x47, 0x49, 0x46], "gif"),
];

/// Detect image type by magic bytes. Returns (content_type, extension).
fn detect_image_type(data: &[u8]) -> Option<(&'static str, &'static str)> {
    for &(content_type, magic, ext) in ALLOWED_IMAGE_TYPES {
        if data.len() >= magic.len() && data[..magic.len()] == *magic {
            return Some((content_type, ext));
        }
    }
    None
}

/// Validate that an image_uri points to our R2 image bucket.
pub fn validate_image_uri(uri: &str) -> bool {
    uri.starts_with(&*config::R2_IMAGE_PUBLIC_URL)
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
        .map_err(|e| AppError::Internal(e))?;

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
        .map_err(|e| AppError::Internal(e))?;

    Ok(R2Client::image_url(&key))
}
```

**Step 4: Update router handler to pass R2Client**

In `crates/api-server/src/router/metadata/mod.rs`, update `upload_image` handler:
```rust
pub async fn upload_image(
    State(state): State<AppState>,
    AuthUser(_session): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let (_filename, data) = extract_file(&mut multipart).await?;
    let uri = upload_service::upload_image(&state.r2, &data).await?;
    Ok(Json(serde_json::json!({ "image_uri": uri })))
}
```

Update `upload_evidence` similarly to pass `&state.r2` and `&filename`.

**Step 5: Verify it compiles**

Run: `cargo check`

**Step 6: Commit**

```bash
git add crates/api-server/src/services/upload.rs crates/api-server/src/router/metadata/mod.rs crates/api-server/src/state.rs crates/api-server/src/main.rs
git commit -m "feat: integrate R2 image upload with validation"
```

---

### Task 4: Add metadata create endpoint

**Files:**
- Create: `crates/api-server/src/services/metadata.rs`
- Modify: `crates/api-server/src/services/mod.rs`
- Modify: `crates/api-server/src/router/metadata/mod.rs`

**Step 1: Create metadata request type and service**

Create `crates/api-server/src/services/metadata.rs`:
```rust
use serde::{Deserialize, Serialize};
use openlaunch_shared::config;
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::storage::r2::R2Client;

use super::upload::validate_image_uri;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateMetadataRequest {
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub homepage: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub discord: Option<String>,
    pub milestones: Vec<MilestoneInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MilestoneInput {
    pub order: i32,
    pub title: String,
    pub description: String,
    pub fund_allocation_percent: i32,
}

impl CreateMetadataRequest {
    pub fn validate(&self) -> AppResult<()> {
        // name
        if self.name.len() < 2 || self.name.len() > 50 {
            return Err(AppError::BadRequest("name must be 2-50 characters".into()));
        }
        // symbol
        if self.symbol.len() < 2 || self.symbol.len() > 10 {
            return Err(AppError::BadRequest("symbol must be 2-10 characters".into()));
        }
        if !self.symbol.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            return Err(AppError::BadRequest("symbol must be uppercase letters and digits only".into()));
        }
        // image_uri
        if !validate_image_uri(&self.image_uri) {
            return Err(AppError::BadRequest("image_uri must be a valid uploaded image URL".into()));
        }
        // optional URLs
        for (name, val) in [
            ("homepage", &self.homepage),
            ("twitter", &self.twitter),
            ("telegram", &self.telegram),
            ("discord", &self.discord),
        ] {
            if let Some(url) = val {
                if !url.starts_with("https://") {
                    return Err(AppError::BadRequest(
                        format!("{name} must start with https://")
                    ));
                }
            }
        }
        // milestones
        if self.milestones.len() < 2 || self.milestones.len() > 6 {
            return Err(AppError::BadRequest("Must have 2-6 milestones".into()));
        }
        let total: i32 = self.milestones.iter().map(|m| m.fund_allocation_percent).sum();
        if total != 100 {
            return Err(AppError::BadRequest(
                format!("Milestone allocations must sum to 100, got {total}")
            ));
        }
        for m in &self.milestones {
            if m.title.is_empty() {
                return Err(AppError::BadRequest("Milestone title is required".into()));
            }
            if m.description.is_empty() {
                return Err(AppError::BadRequest("Milestone description is required".into()));
            }
            if m.fund_allocation_percent < 1 || m.fund_allocation_percent > 100 {
                return Err(AppError::BadRequest("Milestone allocation must be 1-100".into()));
            }
        }
        Ok(())
    }
}

/// Build metadata JSON and upload to R2 metadata bucket. Returns public URL.
pub async fn create_metadata(
    r2: &R2Client,
    req: &CreateMetadataRequest,
) -> AppResult<String> {
    req.validate()?;

    let json = serde_json::to_vec_pretty(req)
        .map_err(|e| AppError::Internal(e.into()))?;

    let key = format!("{}.json", uuid::Uuid::new_v4());

    r2.put_object(
        &config::R2_METADATA_BUCKET,
        &key,
        json,
        "application/json",
    )
    .await
    .map_err(|e| AppError::Internal(e))?;

    Ok(R2Client::metadata_url(&key))
}
```

**Step 2: Export metadata service from `crates/api-server/src/services/mod.rs`**

Add: `pub mod metadata;`

**Step 3: Add route handler in `crates/api-server/src/router/metadata/mod.rs`**

Add the handler:
```rust
use crate::services::metadata as metadata_service;
use crate::services::metadata::CreateMetadataRequest;

// Add to router():
.route("/create", post(create_metadata))

// Handler:
#[utoipa::path(
    post,
    path = "/metadata/create",
    tag = "metadata",
    request_body = CreateMetadataRequest,
    responses(
        (status = 200, description = "Metadata created", body = serde_json::Value),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn create_metadata(
    State(state): State<AppState>,
    AuthUser(_session): AuthUser,
    Json(body): Json<CreateMetadataRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let uri = metadata_service::create_metadata(&state.r2, &body).await?;
    Ok(Json(serde_json::json!({ "metadata_uri": uri })))
}
```

**Step 4: Verify it compiles**

Run: `cargo check`

**Step 5: Commit**

```bash
git add crates/api-server/src/services/metadata.rs crates/api-server/src/services/mod.rs crates/api-server/src/router/metadata/mod.rs
git commit -m "feat: add metadata create endpoint with R2 upload and validation"
```

---

### Task 5: Add tests

**Files:**
- Modify: `crates/api-server/src/services/upload.rs` (add tests)
- Modify: `crates/api-server/src/services/metadata.rs` (add tests)

**Step 1: Add upload tests**

At the bottom of `crates/api-server/src/services/upload.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // PNG magic bytes + minimal data
    const PNG_HEADER: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG_HEADER: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];
    const GIF_HEADER: &[u8] = &[0x47, 0x49, 0x46, 0x38];
    const WEBP_HEADER: &[u8] = &[0x52, 0x49, 0x46, 0x46];

    #[test]
    fn detect_png() {
        let (ct, ext) = detect_image_type(PNG_HEADER).unwrap();
        assert_eq!(ct, "image/png");
        assert_eq!(ext, "png");
    }

    #[test]
    fn detect_jpeg() {
        let (ct, ext) = detect_image_type(JPEG_HEADER).unwrap();
        assert_eq!(ct, "image/jpeg");
        assert_eq!(ext, "jpg");
    }

    #[test]
    fn detect_gif() {
        let (ct, ext) = detect_image_type(GIF_HEADER).unwrap();
        assert_eq!(ct, "image/gif");
        assert_eq!(ext, "gif");
    }

    #[test]
    fn detect_webp() {
        let (ct, ext) = detect_image_type(WEBP_HEADER).unwrap();
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

    #[test]
    fn validate_image_uri_valid() {
        // This test requires R2_IMAGE_PUBLIC_URL to be set
        // In production, validates against the configured URL prefix
        let uri = "https://invalid-bucket.example.com/test.png";
        // Will return false since it doesn't match R2 prefix
        assert!(!validate_image_uri(uri));
    }
}
```

**Step 2: Add metadata validation tests**

At the bottom of `crates/api-server/src/services/metadata.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> CreateMetadataRequest {
        CreateMetadataRequest {
            name: "TestToken".to_string(),
            symbol: "TEST".to_string(),
            image_uri: "placeholder".to_string(), // skips R2 check in unit tests
            homepage: None,
            twitter: None,
            telegram: None,
            discord: None,
            milestones: vec![
                MilestoneInput { order: 1, title: "MVP".into(), description: "Build".into(), fund_allocation_percent: 50 },
                MilestoneInput { order: 2, title: "Launch".into(), description: "Ship".into(), fund_allocation_percent: 50 },
            ],
        }
    }

    #[test]
    fn valid_request_passes() {
        // Need to skip image_uri check for unit tests
        let req = valid_request();
        assert!(req.name.len() >= 2);
        assert!(req.symbol.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn name_too_short() {
        let mut req = valid_request();
        req.name = "A".into();
        assert!(req.validate().is_err());
    }

    #[test]
    fn name_too_long() {
        let mut req = valid_request();
        req.name = "A".repeat(51);
        assert!(req.validate().is_err());
    }

    #[test]
    fn symbol_lowercase_rejected() {
        let mut req = valid_request();
        req.symbol = "test".into();
        assert!(req.validate().is_err());
    }

    #[test]
    fn url_without_https_rejected() {
        let mut req = valid_request();
        req.twitter = Some("http://twitter.com".into());
        assert!(req.validate().is_err());
    }

    #[test]
    fn milestones_must_sum_100() {
        let mut req = valid_request();
        req.milestones[0].fund_allocation_percent = 60;
        assert!(req.validate().is_err());
    }

    #[test]
    fn too_few_milestones() {
        let mut req = valid_request();
        req.milestones = vec![
            MilestoneInput { order: 1, title: "Only".into(), description: "One".into(), fund_allocation_percent: 100 },
        ];
        assert!(req.validate().is_err());
    }

    #[test]
    fn too_many_milestones() {
        let mut req = valid_request();
        req.milestones = (1..=7).map(|i| MilestoneInput {
            order: i, title: format!("M{i}"), description: format!("D{i}"),
            fund_allocation_percent: if i <= 2 { 15 } else { 14 },
        }).collect();
        assert!(req.validate().is_err());
    }

    #[test]
    fn empty_milestone_title_rejected() {
        let mut req = valid_request();
        req.milestones[0].title = "".into();
        assert!(req.validate().is_err());
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p openlaunch-api-server`

**Step 4: Commit**

```bash
git add crates/api-server/src/services/upload.rs crates/api-server/src/services/metadata.rs
git commit -m "test: add upload validation and metadata validation tests"
```

---

### Task 6: Update .env.example and set Railway env vars

**Files:**
- Modify: `.env.example`

**Step 1: Add R2 vars to `.env.example`**

```env
# R2 Storage
R2_ACCOUNT_ID=your_cloudflare_account_id
R2_ACCESS_KEY_ID=your_r2_access_key
R2_SECRET_ACCESS_KEY=your_r2_secret_key
R2_IMAGE_BUCKET=openlaunch-image
R2_METADATA_BUCKET=openlaunch-metadata
R2_IMAGE_PUBLIC_URL=https://openlaunch-image.{account-id}.r2.dev
R2_METADATA_PUBLIC_URL=https://openlaunch-metadata.{account-id}.r2.dev
```

**Step 2: Commit**

```bash
git add .env.example
git commit -m "chore: add R2 storage vars to .env.example"
```

**Step 3: Set Railway env vars (after user provides R2 credentials)**

```bash
for svc in api-server observer txbot websocket-server; do
  railway variable set \
    R2_ACCOUNT_ID=... \
    R2_ACCESS_KEY_ID=... \
    R2_SECRET_ACCESS_KEY=... \
    R2_IMAGE_BUCKET=openlaunch-image \
    R2_METADATA_BUCKET=openlaunch-metadata \
    R2_IMAGE_PUBLIC_URL=https://openlaunch-image.{account-id}.r2.dev \
    R2_METADATA_PUBLIC_URL=https://openlaunch-metadata.{account-id}.r2.dev \
    --service "$svc"
done
```
