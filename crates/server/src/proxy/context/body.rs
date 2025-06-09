use std::pin::Pin;
use std::task;

use bytes::Bytes;
use http::response::Parts as ResponseParts;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Frame, Incoming};
use tower_http::decompression::DecompressionBody;

#[pin_project::pin_project(project = ProxyBodyProj)]
pub enum ProxyBody {
    Uncompressed(#[pin] Full<Bytes>),
    Compressed(#[pin] Box<DecompressionBody<Full<Bytes>>>),
}

impl ProxyBody {
    pub async fn uncompressed(body: Incoming) -> anyhow::Result<Self> {
        body.collect()
            .await
            .map(|body| {
                let data = body.to_bytes();
                Self::Uncompressed(Full::new(data))
            })
            .map_err(Into::into)
    }

    pub async fn compressed(parts: &ResponseParts, body: Incoming) -> anyhow::Result<Self> {
        use http_body_util::BodyExt;
        use tower::Service;
        use tower_http::decompression::Decompression;

        let moke_req = http::Request::builder().body(())?;

        let data = body.collect().await?.to_bytes();
        let res = http::Response::from_parts(parts.clone(), Full::new(data));

        let mut decompression = Decompression::new(tower::service_fn(|_| {
            let res = res.clone();
            futures::future::ok::<_, std::convert::Infallible>(res)
        }));

        let (_, body) = decompression.call(moke_req).await?.into_parts();

        Ok(Self::Compressed(Box::new(body)))
    }

    #[allow(unused)]
    pub fn is_uncompressed(&self) -> bool {
        matches!(self, Self::Uncompressed(_))
    }

    #[allow(unused)]
    pub fn is_compressed(&self) -> bool {
        matches!(self, Self::Compressed(_))
    }

    pub fn len(&self) -> usize {
        // It's safe to unwrap here since we already have the full body
        self.size_hint().exact().expect("should not be none") as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Get the full (uncompressed) body
    pub async fn collect_all(self) -> anyhow::Result<Bytes> {
        Ok(self.collect().await?.to_bytes())
    }

    // Get the full (possibly compressed) body
    pub async fn collect_raw(self) -> anyhow::Result<Bytes> {
        let body = match self {
            Self::Uncompressed(body) => body,
            Self::Compressed(body) => body.into_inner(),
        };

        Ok(body.collect().await?.to_bytes())
    }
}

impl Body for ProxyBody {
    type Data = Bytes;
    type Error = anyhow::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.project() {
            ProxyBodyProj::Uncompressed(body) => body.poll_frame(cx).map_err(Into::into),
            ProxyBodyProj::Compressed(body) => {
                body.poll_frame(cx).map_err(|err| anyhow::anyhow!(err))
            }
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        match self {
            Self::Uncompressed(ref body) => body.size_hint(),
            Self::Compressed(ref body) => body.size_hint(),
        }
    }
}
