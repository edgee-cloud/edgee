#[derive(Debug)]
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
pub(crate) fn parse_html(html: &str) -> Document {
    let recorded_tags: Vec<&str> = vec!["script", "title", "meta", "link"];
    let mut results = Document {
        data_collection_events: String::new(),
        sdk_full_tag: String::new(),
        sdk_src: String::new(),
        inlined_sdk: String::new(),
        data_layer: String::new(),
        title: String::new(),
        canonical: String::new(),
        keywords: String::new(),
    };
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
                    for tag in recorded_tags.iter() {
                        if next_chars.starts_with(tag) {
                            recording = true;
                            temp.clear();
                            break;
                        }
                    }
                }
                temp.push(c);
            }
            '>' if recording => {
                temp.push(c);

                if temp.contains(r#"__EDGEE_SDK__"#) {
                    if temp.ends_with("/>") {
                        results.sdk_full_tag = temp.clone();
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
                        results.sdk_src = extract_src_value(&temp).unwrap_or_default();

                        // check if data-inline="false" is present
                        let inline = !temp.contains(r#"data-inline="false""#);

                        // if inline is true, then we need to inline the SDK
                        if inline && !results.sdk_src.is_empty() {
                            let inlined_sdk = get_sdk_from_url(&results.sdk_src);
                            if inlined_sdk.is_ok() {
                                results.inlined_sdk = inlined_sdk.unwrap();
                            }
                        }
                        results.sdk_full_tag = temp.clone();
                    }
                } else if temp.contains(r#"__EDGEE_DATA_LAYER__"#) {
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
                    results.data_layer = temp.clone();
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
                    results.title = title_tag;
                } else if temp.contains(r#"rel="canonical""#) {
                    // get only what is in the href attribute
                    let href = extract_href_value(&temp);
                    if href.is_some() {
                        results.canonical = href.unwrap();
                    }
                } else if temp.contains(r#"name="keywords""#) {
                    // get only what is in the content attribute
                    let content = extract_content_value(&temp);
                    if content.is_some() {
                        results.keywords = content.unwrap();
                    }
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

    results
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
/// ```
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
/// ```
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
/// ```
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

/// Retrieves the SDK content from a given URL.
///
/// This function extracts the version of the SDK from the URL using a regular expression.
/// Then, it retrieves the SDK content based on the extracted version.
/// The function returns a `Result` that contains the SDK content wrapped in a `<script>` tag if successful, or an error message if not.
///
/// # Arguments
///
/// * `url` - A string slice that holds the URL.
///
/// # Returns
///
/// * `Result<String, &'static str>` - The SDK content wrapped in a `<script>` tag if successful, or an error message if not.
///
/// # Example
///
/// ```
/// let url = "https://example.com/sdk/v1.1.0.js";
/// let sdk_content = get_sdk_from_url(url);
/// assert!(sdk_content.is_ok());
/// ```
pub fn get_sdk_from_url(url: &str) -> Result<String, &'static str> {
    if url.ends_with("sdk.js") {
        return Ok(include_str!("../../../public/sdk.js").trim().to_string());
    }

    let Some((_, part)) = url.rsplit_once("edgee.v") else {
        return Err("Failed to read the JS SDK file");
    };
    let Some(part) = part.strip_suffix(".js") else {
        return Err("Failed to read the JS SDK file");
    };

    let content = match part {
        "1.1.0" => include_str!("../../../public/edgee.v1.1.0.js"),
        // Add more versions as needed
        _ => return Err("Failed to read the JS SDK file"),
    };

    Ok(content.trim().to_string())
}
