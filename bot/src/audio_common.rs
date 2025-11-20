use crate::{AppContext, CachedFile, context_ext::ContextExt};
use fruityger::format::Format;
use grammers_client::grammers_tl_types::enums::InputBotInlineMessageId;
use mystbot_core::inline_message_ext::InlineMessageExt;
use tokio::sync::mpsc;

pub type DownloadFunc<T, F> = fn(AppContext, T, mpsc::Sender<String>, bool) -> F;

pub struct DownloadedTrack {
    pub track_file: CachedFile,
    pub cover_file: Option<CachedFile>,
}

async fn get_cover(context: AppContext, url: &str) -> anyhow::Result<CachedFile> {
    let dir = tempfile::tempdir()?;
    let response = reqwest::get(url).await?;
    let (path, format) = fruityger::save_cover(response, dir.path(), "cover").await?;
    context
        .upload_cached_file(url, &path, format.mime_type())
        .await
}

pub async fn get_downloaded_track<
    Fut: Future<Output = anyhow::Result<CachedFile>>,
    Func: FnOnce(AppContext) -> Fut,
>(
    context: AppContext,
    refresh: bool,
    track_url: String,
    cover_url: Option<String>,
    get_track: Func,
) -> anyhow::Result<DownloadedTrack> {
    let track_file = if refresh {
        get_track(context.clone()).await?
    } else {
        match context.get_cached_file(&track_url).await {
            Some(audio) => audio,
            None => get_track(context.clone()).await?,
        }
    };

    let cover_file = if let Some(cover_url) = cover_url {
        Some(if refresh {
            get_cover(context.clone(), &cover_url).await?
        } else {
            match context.get_cached_file(&cover_url).await {
                Some(artwork) => artwork,
                None => get_cover(context.clone(), &cover_url).await?,
            }
        })
    } else {
        None
    };

    Ok(DownloadedTrack {
        track_file,
        cover_file,
    })
}

pub async fn retry_send_inline_with_progress<
    T: Clone,
    F: Future<Output = anyhow::Result<DownloadedTrack>>,
>(
    context: AppContext,
    message_id: InputBotInlineMessageId,
    title: String,
    artist: String,
    duration_ms: u64,
    data: T,
    download_func: DownloadFunc<T, F>,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(16);
    {
        let message_id = message_id.clone();
        let client = context.client.clone();
        tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                let _ = client
                    .edit_inline_message_ext(
                        message_id.clone(),
                        m,
                        Some("Скачиваем...".to_string()),
                        None,
                    )
                    .await;
            }
        });
    }

    let mut sent = false;
    for i in 0..2 {
        if let Ok(downloaded_track) =
            download_func(context.clone(), data.clone(), tx.clone(), i == 1).await
        {
            sent = context
                .send_downloaded_track(
                    downloaded_track,
                    message_id.clone(),
                    title.clone(),
                    artist.clone(),
                    duration_ms,
                )
                .await?;
            if sent {
                break;
            }
        };
    }

    if !sent {
        context
            .client
            .edit_inline_message_ext(message_id.clone(), "Не удалось скачать трек", None, None)
            .await?;
    }

    Ok(())
}
