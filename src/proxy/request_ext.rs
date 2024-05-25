use http::{Request, Uri};

pub trait RequestExt {
    fn is_http(&self) -> bool;
    fn https_uri(&self) -> Uri;
}

impl<T> RequestExt for Request<T> {
    fn is_http(&self) -> bool {
        self.uri().scheme().unwrap() == "http"
    }

    fn https_uri(&self) -> Uri {
        let mut uri_parts = self.uri().clone().into_parts();
        uri_parts.scheme = Some("https".parse().unwrap());
        Uri::from_parts(uri_parts).unwrap()
    }
}
