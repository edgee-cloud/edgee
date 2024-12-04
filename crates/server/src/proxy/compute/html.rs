#[derive(Debug, Default)]
pub struct Document {
    pub data_collection_events: String,
    pub sdk_full_tag: String,
    pub sdk_src: String,
    pub inlined_sdk: String,
    pub data_layer: String,
    pub title: String,
    pub canonical: String,
    pub keywords: String,
}

#[derive(Debug, Default)]
struct DocumentBuilder {
    sdk_full_tag: Option<String>,
    sdk_src: Option<String>,
    inlined_sdk: Option<String>,
    data_layer: Option<String>,
    title: Option<String>,
    canonical: Option<String>,
    keywords: Option<String>,
}

impl DocumentBuilder {
    fn is_complete(&self) -> bool {
        matches!(
            *self,
            DocumentBuilder {
                sdk_full_tag: Some(_),
                sdk_src: Some(_),
                inlined_sdk: Some(_),
                data_layer: Some(_),
                title: Some(_),
                canonical: Some(_),
                keywords: Some(_),
            }
        )
    }

    fn build(self) -> Document {
        Document {
            sdk_full_tag: self.sdk_full_tag.unwrap_or_default(),
            sdk_src: self.sdk_src.unwrap_or_default(),
            inlined_sdk: self.inlined_sdk.unwrap_or_default(),
            data_layer: self.data_layer.unwrap_or_default(),
            title: self.title.unwrap_or_default(),
            canonical: self.canonical.unwrap_or_default(),
            keywords: self.keywords.unwrap_or_default(),
            ..Default::default()
        }
    }
}

macro_rules! set_document_field {
    ($builder:expr, $field:ident, $value:expr) => {
        set_document_field!($builder, $field, ?Some($value));
    };
    ($builder:expr, $field:ident, ?$value:expr) => {
        $builder.$field = $value;
        if $builder.is_complete() {
            return $builder.build();
        }
    };
}

/// Parses an HTML document and extracts specific information.
///
/// This function parses an HTML document and extracts the following information:
/// - The Edgee trace UUID.
/// - The Edgee SDK tag
/// - The Edgee opened SDK tag
/// - The inlined SDK content, only if the `data-inline` attribute is not set to `false`.
/// - The Edgee payload content if it exists.
/// - The title tag and its content.
/// - The canonical link tag and its `href` attribute value.
/// - The keywords meta tag and its `content` attribute value.
///
/// The function returns a `Document` struct containing the extracted information.
///
/// # Arguments
///
/// * `html` - A string slice that holds the HTML document.
///
/// # Returns
///
/// * `Document` - A struct containing the extracted information.
///
pub fn parse_html(html: &str, host: &str) -> Document {
    static RECORDED_TAGS: &[&str] = &["script", "title", "meta", "link"];

    let mut builder = DocumentBuilder::default();
    if !html.contains("__EDGEE_DATA_LAYER__") {
        builder.data_layer = Some(String::new());
    }

    let mut temp = String::new();
    let mut recording = false;

    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '<' if chars.peek() == Some(&'!') => {
                chars.next(); // Consume '!'
                if chars.peek() == Some(&'-') {
                    chars.next(); // Consume '-'
                    if chars.peek() == Some(&'-') {
                        chars.next(); // Consume '-'
                                      // Start of a comment

                        while let Some(&next_c) = chars.peek() {
                            chars.next(); // Consume character
                            temp.push(next_c);
                            if next_c == '>' && temp.ends_with("-->") {
                                break;
                            }
                        }
                        temp.clear(); // Clear temporary storage
                    }
                }
            }
            '<' => {
                let next_chars: String = chars.clone().take(6).collect();
                if !recording {
                    // if next_chars start with RECORDED_TAGS list
                    for tag in RECORDED_TAGS.iter() {
                        if next_chars.starts_with(tag) {
                            recording = true;
                            temp.clear();
                            break;
                        }
                    }
                }
                if next_chars.starts_with("/head") {
                    builder.title.get_or_insert_with(String::new);
                    builder.canonical.get_or_insert_with(String::new);
                    builder.keywords.get_or_insert_with(String::new);
                    if builder.is_complete() {
                        return builder.build();
                    }
                }
                temp.push(c);
            }
            '>' if recording => {
                temp.push(c);

                if temp.contains("__EDGEE_SDK__") {
                    if temp.ends_with("/>") {
                        set_document_field!(builder, sdk_full_tag, temp.clone());
                    } else {
                        // This is a start tag, so we need to get the full tag with the closing tag as well
                        while let Some(&next_c) = chars.peek() {
                            chars.next(); // Consume character
                            temp.push(next_c);
                            if next_c == '>' {
                                // check if it is really the closing bracket
                                if temp.ends_with("script>") {
                                    break;
                                }
                            }
                        }
                        // get only what is in the src attribute
                        set_document_field!(builder, sdk_src, ?extract_src_value(&temp));

                        // check if data-inline="false" is present
                        let inline = !temp.contains(r#"data-inline="false""#);

                        // if inline is true, then we need to inline the SDK
                        if let (true, Some(sdk_url)) = (inline, &builder.sdk_src) {
                            if let Ok(inlined_sdk) = edgee_sdk::get_sdk_from_url(sdk_url, host) {
                                set_document_field!(builder, inlined_sdk, inlined_sdk);
                            }
                        }
                        set_document_field!(builder, sdk_full_tag, temp.clone());
                    }
                } else if temp.contains("__EDGEE_DATA_LAYER__") {
                    // first, remove the opening tag
                    temp.clear();

                    // This is the start tag of Edgee payload, so we need to get the full tag with the closing tag as well
                    while let Some(&next_c) = chars.peek() {
                        chars.next(); // Consume character
                        temp.push(next_c);
                        if next_c == '>' {
                            // check if it is really the closing bracket
                            if temp.ends_with("script>") {
                                break;
                            }
                        }
                    }

                    // then replace the closing tag </script> by an empty string
                    temp = temp.replace("</script>", "");

                    // get only what is between the tags
                    set_document_field!(builder, data_layer, temp.clone());
                } else if temp == "<title>" {
                    // This is the start tag of the title, so we need to get the full tag with the closing tag as well
                    while let Some(&next_c) = chars.peek() {
                        chars.next(); // Consume character
                        temp.push(next_c);
                        if next_c == '>' {
                            // check if it is really the closing bracket
                            if temp.ends_with("title>") {
                                break;
                            }
                        }
                    }
                    // get only what is between the tags
                    let mut title_tag = temp.clone();
                    title_tag = title_tag.replace("</title>", "");
                    title_tag = title_tag.replace("<title>", "");
                    set_document_field!(builder, title, title_tag);
                } else if temp.contains(r#"rel="canonical""#) {
                    // get only what is in the href attribute
                    set_document_field!(builder, canonical, ?extract_href_value(&temp));
                } else if temp.contains(r#"name="keywords""#) {
                    // get only what is in the content attribute
                    set_document_field!(builder, keywords, ?extract_content_value(&temp));
                }

                recording = false;
                temp.clear();
            }
            _ if recording => {
                temp.push(c);
            }
            _ => {}
        }
    }

    builder.build()
}

/// Extracts the value of the `href` attribute from a given HTML tag.
///
/// # Arguments
///
/// * `tag` - A string slice that holds the HTML tag.
///
/// # Returns
///
/// * `Option<String>` - The value of the `href` attribute if it exists, `None` otherwise.
///
/// # Example
///
/// ```ignore
/// let tag = r#"<a href="https://example.com">Example</a>"#;
/// let href_value = extract_href_value(tag);
/// assert_eq!(href_value, Some("https://example.com".to_string()));
/// ```
fn extract_href_value(tag: &str) -> Option<String> {
    // Look for the start of the href attribute
    let start = tag.find(r#"href=""#)?;

    // We add 6 to move past 'href="' to the start of the actual value
    let rest_of_tag = &tag[start + 6..];

    // Now, find the position of the closing quote
    let end_quote = rest_of_tag.find('"')?;

    // Extract the value between the quotes
    Some(rest_of_tag[..end_quote].to_string())
}

/// Extracts the value of the `src` attribute from a given HTML tag.
///
/// # Arguments
///
/// * `tag` - A string slice that holds the HTML tag.
///
/// # Returns
///
/// * `Option<String>` - The value of the `src` attribute if it exists, `None` otherwise.
///
/// # Example
///
/// ```ignore
/// let tag = r#"<img src="https://example.com/image.jpg">"#;
/// let src_value = extract_src_value(tag);
/// assert_eq!(src_value, Some("https://example.com/image.jpg".to_string()));
/// ```
fn extract_src_value(tag: &str) -> Option<String> {
    // Look for the start of the src attribute
    let start = tag.find(r#"src=""#)?;

    // We add 5 to move past 'src="' to the start of the actual value
    let rest_of_tag = &tag[start + 5..];

    // Now, find the position of the closing quote
    let end_quote = rest_of_tag.find('"')?;

    // Extract the value between the quotes
    Some(rest_of_tag[..end_quote].to_string())
}

/// Extracts the value of the `content` attribute from a given HTML tag.
///
/// # Arguments
///
/// * `tag` - A string slice that holds the HTML tag.
///
/// # Returns
///
/// * `Option<String>` - The value of the `content` attribute if it exists, `None` otherwise.
///
/// # Example
///
/// ```ignore
/// let tag = r#"<meta name="description" content="This is an example.">"#;
/// let content_value = extract_content_value(tag);
/// assert_eq!(content_value, Some("This is an example.".to_string()));
/// ```
fn extract_content_value(tag: &str) -> Option<String> {
    // Look for the start of the content attribute
    let start = tag.find(r#"content=""#)?;

    // We add 9 to move past 'content="' to the start of the actual value
    let rest_of_tag = &tag[start + 9..];

    // Now, find the position of the closing quote
    let end_quote = rest_of_tag.find('"')?;

    // Extract the value between the quotes
    Some(rest_of_tag[..end_quote].to_string())
}
