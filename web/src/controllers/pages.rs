use askama::Template;
use axum::response::Html;

#[derive(Template)]
#[template(path = "privacy.html")]
pub struct PrivacyTemplate;

pub async fn privacy() -> Html<String> {
    let template = PrivacyTemplate;
    Html(template.render().unwrap())
}
