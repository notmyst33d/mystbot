// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Myst33d <myst33d@gmail.com>

use grammers_client::{
    grammers_tl_types::{Cursor, Deserializable, Serializable, enums::InputFile},
    types::media::Uploaded,
};
use mystbot_core::client_wrapper::ClientWrapper;
use std::path::Path;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

/// Cached file struct which consists of [Uploaded] and content type
pub struct CachedFile(pub Uploaded, pub String);

fn cache_key(url: &str, name: &str) -> String {
    format!("++media+{url}+{name}")
        .replace("/", "+")
        .replace(":", "+")
}

/// Returns [Uploaded] and content type of a cached file
pub async fn get_cached_file(cache_dir: &Path, url: &str) -> Option<CachedFile> {
    let uploaded = {
        let mut v = Vec::new();
        let mut file = File::open(cache_dir.join(cache_key(url, "packed")))
            .await
            .ok()?;
        file.read_to_end(&mut v).await.ok()?;
        Uploaded::from_raw(InputFile::deserialize(&mut Cursor::from_slice(&v)).ok()?)
    };
    let content_type = {
        let mut s = String::new();
        let mut file = File::open(cache_dir.join(cache_key(url, "content_type")))
            .await
            .ok()?;
        file.read_to_string(&mut s).await.ok()?;
        s
    };
    Some(CachedFile(uploaded, content_type))
}

pub async fn upload_cached_file(
    client: ClientWrapper,
    cache_dir: &Path,
    path: &Path,
    url: &str,
    content_type: &str,
) -> Option<CachedFile> {
    let uploaded = client.upload_file(path).await.ok()?;

    let mut file = File::create(cache_dir.join(cache_key(url, "packed")))
        .await
        .ok()?;
    file.write_all(&uploaded.raw.to_bytes()).await.ok()?;

    let mut file = File::create(cache_dir.join(cache_key(url, "content_type")))
        .await
        .ok()?;
    file.write_all(content_type.as_bytes()).await.ok()?;

    Some(CachedFile(uploaded, content_type.to_string()))
}
