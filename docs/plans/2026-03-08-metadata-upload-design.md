# Metadata & Image Upload Design

## Goal

Upload project images and metadata to Cloudflare R2, with strict type validation and R2-only image_uri enforcement.

## R2 Buckets

| Bucket | Purpose | Key Pattern |
|--------|---------|-------------|
| `openlaunch-image` | Project images | `{uuid}.{ext}` |
| `openlaunch-metadata` | Project metadata JSON | `{uuid}.json` |

Both buckets have public access enabled. URLs follow the pattern `https://{bucket}.{account-id}.r2.dev/{key}`.

## API Endpoints

### POST /metadata/image (multipart)

Auth required. Uploads an image to the R2 image bucket.

**Validation:**
- Content-Type: `image/png`, `image/jpeg`, `image/webp`, `image/gif` only
- File size: 5MB max
- Magic bytes verification (double-check actual file type)

**Response:**
```json
{ "image_uri": "https://openlaunch-image.{account-id}.r2.dev/{uuid}.png" }
```

### POST /metadata/create (JSON)

Auth required. Creates a metadata JSON and uploads to R2 metadata bucket.

**Request body:**
```json
{
  "name": "MyToken",
  "symbol": "MTK",
  "image_uri": "https://openlaunch-image.{...}.r2.dev/xxx.png",
  "category": "DeFi",
  "homepage": "https://...",
  "twitter": "https://...",
  "telegram": "https://...",
  "discord": "https://...",
  "milestones": [
    { "order": 1, "title": "MVP", "description": "Build MVP", "fund_allocation_percent": 50 },
    { "order": 2, "title": "Launch", "description": "Ship", "fund_allocation_percent": 50 }
  ]
}
```

**Response:**
```json
{ "metadata_uri": "https://openlaunch-metadata.{account-id}.r2.dev/{uuid}.json" }
```

## Validation Rules

| Field | Type | Rule |
|-------|------|------|
| name | String | Required, 2-50 chars |
| symbol | String | Required, 2-10 chars, `[A-Z0-9]` only |
| image_uri | String | Required, must start with R2 image bucket URL prefix |
| category | String | Required, 1-50 chars, free text |
| homepage | Option\<String\> | If present, must start with `https://` |
| twitter | Option\<String\> | If present, must start with `https://` |
| telegram | Option\<String\> | If present, must start with `https://` |
| discord | Option\<String\> | If present, must start with `https://` |
| milestones | Vec | 2-6 items, allocations sum to 100% |

### Image URI Validation

`image_uri` is checked against the `R2_IMAGE_PUBLIC_URL` env var prefix. Only images uploaded through our `/metadata/image` endpoint are accepted.

## Environment Variables

```
R2_ACCOUNT_ID
R2_ACCESS_KEY_ID
R2_SECRET_ACCESS_KEY
R2_IMAGE_BUCKET=openlaunch-image
R2_METADATA_BUCKET=openlaunch-metadata
R2_IMAGE_PUBLIC_URL=https://openlaunch-image.{account-id}.r2.dev
R2_METADATA_PUBLIC_URL=https://openlaunch-metadata.{account-id}.r2.dev
```

## Flow

1. User uploads image via `POST /metadata/image` -> gets `image_uri`
2. User submits metadata via `POST /metadata/create` with `image_uri` and other fields
3. Server validates `image_uri` starts with our R2 image bucket URL
4. Server builds metadata JSON, uploads to R2 metadata bucket as `{uuid}.json`
5. Returns `metadata_uri` pointing to the metadata JSON

## Tech

- `aws-sdk-s3` crate (S3-compatible, works with R2)
- `uuid` crate for key generation
- Magic bytes check via first few bytes of uploaded file
