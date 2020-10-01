use core::pin::Pin;
use core::task::{Context, Poll};
use futures::Stream;
use pin_project::pin_project;

#[pin_project]
pub struct Adapter {
    #[pin]
    inner:
        Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + Sync + 'static>>,
}

impl Adapter {
    pub fn new<S>(stream: S) -> Adapter
    where
        S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + Sync + 'static,
    {
        Adapter {
            inner: Box::pin(stream),
        }
    }
}

impl futures::Stream for Adapter {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner
            .poll_next(cx)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        return self.inner.size_hint();
    }
}
