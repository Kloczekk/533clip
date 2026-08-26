use crate::storage::R2Settings;
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::path::Path;
use std::time::Duration;

pub async fn r2_client(settings: &R2Settings) -> Result<Client, String> {
    if settings.access_key_id.is_empty()
        || settings.secret_access_key.is_empty()
        || settings.bucket.is_empty()
    {
        return Err("sharing settings incomplete".into());
    }

    let provider = settings.provider.trim().to_lowercase();
    let endpoint = if provider == "b2" {
        if settings.endpoint_url.trim().is_empty() {
            return Err("Backblaze B2 endpoint missing".into());
        }
        settings.endpoint_url.trim().trim_end_matches('/').to_string()
    } else {
        if settings.account_id.trim().is_empty() {
            return Err("R2 account ID missing".into());
        }
        format!("https://{}.r2.cloudflarestorage.com", settings.account_id)
    };
    let region = if settings.region.trim().is_empty() {
        if provider == "b2" { "us-west-004" } else { "auto" }
    } else {
        settings.region.trim()
    };
    let creds = Credentials::new(
        settings.access_key_id.clone(),
        settings.secret_access_key.clone(),
        None,
        None,
        if provider == "b2" { "b2" } else { "r2" },
    );
    let builder = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region.to_string()))
        .endpoint_url(endpoint)
        .credentials_provider(creds);
    let config = builder.load().await;
    Ok(Client::new(&config))
}

pub async fn upload_file(settings: &R2Settings, key: &str, path: &Path) -> Result<String, String> {
    let client = r2_client(settings).await?;
    let body = ByteStream::from_path(path)
        .await
        .map_err(|e| format!("could not read export: {e}"))?;
    client
        .put_object()
        .bucket(&settings.bucket)
        .key(key)
        .body(body)
        .content_type("video/mp4")
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;

    if settings.public_base_url.is_empty() {
        let days = settings.delete_after_days.clamp(1, 7);
        let expires_in = Duration::from_secs(days as u64 * 24 * 60 * 60);
        let presigned = client
            .get_object()
            .bucket(&settings.bucket)
            .key(key)
            .presigned(
                PresigningConfig::expires_in(expires_in)
                    .map_err(|e| format!("signed link failed: {e}"))?,
            )
            .await
            .map_err(|e| format!("signed link failed: {e}"))?;
        Ok(presigned.uri().to_string())
    } else {
        Ok(format!(
            "{}/{}",
            settings.public_base_url.trim_end_matches('/'),
            key
        ))
    }
}

pub async fn delete_object(settings: &R2Settings, key: &str) -> Result<(), String> {
    let client = r2_client(settings).await?;
    client
        .delete_object()
        .bucket(&settings.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| format!("delete failed: {e}"))?;
    Ok(())
}
