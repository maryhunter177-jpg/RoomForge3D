use super::{internal, ApiError};
use axum::http::StatusCode;
use s3::{bucket::Bucket, creds::Credentials, Region};
use std::env;

#[derive(Clone)]
pub struct Storage {
    bucket: Box<Bucket>,
    signing_bucket: Box<Bucket>,
}
impl Storage {
    pub fn from_env() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Ok(endpoint) = env::var("STORAGE_ENDPOINT") else {
            return Ok(None);
        };
        let credentials = Credentials::new(
            Some(&env::var("STORAGE_ACCESS_KEY")?),
            Some(&env::var("STORAGE_SECRET_KEY")?),
            None,
            None,
            None,
        )?;
        let region = Region::Custom {
            region: env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".into()),
            endpoint,
        };
        let bucket = Bucket::new(&env::var("STORAGE_BUCKET")?, region, credentials.clone())?
            .with_path_style();
        let signing_bucket = match env::var("STORAGE_PUBLIC_ENDPOINT") {
            Ok(endpoint) => Bucket::new(
                &env::var("STORAGE_BUCKET")?,
                Region::Custom {
                    region: env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".into()),
                    endpoint,
                },
                credentials,
            )?
            .with_path_style(),
            Err(_) => bucket.clone(),
        };
        Ok(Some(Self {
            bucket,
            signing_bucket,
        }))
    }
    pub async fn put(&self, key: &str, bytes: &[u8], mime: &str) -> Result<(), ApiError> {
        let mut bucket = self.bucket.clone();
        bucket.add_header("Cache-Control", "public, max-age=31536000, immutable");
        let response = bucket
            .put_object_with_content_type(key, bytes, mime)
            .await
            .map_err(internal)?;
        if !(200..300).contains(&response.status_code()) {
            return Err(ApiError(
                StatusCode::BAD_GATEWAY,
                "Falha no armazenamento de arquivos",
            ));
        }
        Ok(())
    }
    pub async fn url(&self, key: &str) -> Result<String, ApiError> {
        self.signing_bucket
            .presign_get(key, 3600, None)
            .await
            .map_err(internal)
    }
    pub async fn delete(&self, key: &str) -> Result<(), ApiError> {
        let response = self.bucket.delete_object(key).await.map_err(internal)?;
        if !(200..300).contains(&response.status_code()) {
            return Err(ApiError(
                StatusCode::BAD_GATEWAY,
                "Falha ao remover arquivo",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn signs_public_endpoint_without_exposing_internal_hostname() {
        let credentials =
            Credentials::new(Some("test-access"), Some("test-secret"), None, None, None)
                .expect("credentials");
        let bucket = Bucket::new(
            "test-bucket",
            Region::Custom {
                region: "us-east-1".into(),
                endpoint: "http://minio:9000".into(),
            },
            credentials.clone(),
        )
        .expect("internal bucket")
        .with_path_style();
        let signing_bucket = Bucket::new(
            "test-bucket",
            Region::Custom {
                region: "us-east-1".into(),
                endpoint: "http://localhost:9000".into(),
            },
            credentials,
        )
        .expect("public bucket")
        .with_path_style();
        let storage = Storage {
            bucket,
            signing_bucket,
        };
        let url = storage
            .url("assets/test/model.glb")
            .await
            .expect("signed url");
        assert!(url.starts_with("http://localhost:9000/test-bucket/assets/test/model.glb?"));
        assert!(url.contains("X-Amz-Signature="));
        assert!(url.contains("X-Amz-Expires=3600"));
        assert!(!url.contains("minio:9000"));
    }
}
