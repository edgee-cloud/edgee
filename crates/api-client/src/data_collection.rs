use connect_builder::{
    IsUnset, SetBaseurl, SetClientBuilder, SetCookies, SetDefaultHeaders, State,
};

const PROD_DOMAIN: &str = "edgee.app";

progenitor::generate_api! {
    spec = "dc-openapi.json",
    interface = Builder,
    derives = [ schemars::JsonSchema ],
}

#[bon::builder(
    start_fn = new,
    finish_fn = connect,
    on(String, into),
)]
pub fn connect(
    baseurl: String,
    #[builder(default, setters(vis = ""))] client_builder: reqwest::ClientBuilder,
    #[builder(default, setters(vis = ""))] mut default_headers: reqwest::header::HeaderMap,
    #[builder(default, setters(vis = ""))] mut cookies: cookie::CookieJar,
    #[builder(default = false)] debug_mode: bool,
) -> Client {
    use reqwest::header;

    if debug_mode {
        cookies.add(("_edgeedebug", "true"));
    }

    let cookie_header = {
        let entries: Vec<_> = cookies
            .iter()
            .map(|cookie| cookie.stripped().encoded().to_string())
            .collect();

        entries.join("; ")
    };
    default_headers.insert(header::COOKIE, cookie_header.parse().unwrap());

    let client = client_builder
        .user_agent(concat!("edgee/", env!("CARGO_PKG_VERSION")))
        .default_headers(default_headers)
        .build()
        .unwrap();

    Client::new_with_client(&baseurl, client)
}

impl<S: State> ConnectBuilder<S> {
    pub fn with_client_builder(
        self,
        f: impl Fn(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    ) -> ConnectBuilder<SetClientBuilder<S>>
    where
        S::ClientBuilder: IsUnset,
    {
        self.client_builder(f(Default::default()))
    }

    pub fn with_default_headers(
        self,
        f: impl Fn(&mut reqwest::header::HeaderMap),
    ) -> ConnectBuilder<SetDefaultHeaders<S>>
    where
        S::DefaultHeaders: IsUnset,
    {
        let mut headers = Default::default();
        f(&mut headers);
        self.default_headers(headers)
    }

    pub fn with_cookies(self, f: impl Fn(&mut cookie::CookieJar)) -> ConnectBuilder<SetCookies<S>>
    where
        S::Cookies: IsUnset,
    {
        let mut cookies = Default::default();
        f(&mut cookies);
        self.cookies(cookies)
    }

    pub fn project_name(self, name: impl Into<String>) -> ConnectBuilder<SetBaseurl<S>>
    where
        S::Baseurl: IsUnset,
    {
        let project_name = name.into();
        self.baseurl(format!("https://{project_name}.{PROD_DOMAIN}"))
    }
}

impl types::EdgeeEventDataCollectionEventsItem {
    pub fn page(
        builder: types::builder::EdgeeEventPage,
    ) -> Result<Self, types::error::ConversionError> {
        Ok(Self::Page(builder.type_("page").try_into()?))
    }

    pub fn user(
        builder: types::builder::EdgeeEventUser,
    ) -> Result<Self, types::error::ConversionError> {
        Ok(Self::User(builder.type_("user").try_into()?))
    }

    pub fn track(
        builder: types::builder::EdgeeEventTrack,
    ) -> Result<Self, types::error::ConversionError> {
        Ok(Self::Track(builder.type_("track").try_into()?))
    }
}
