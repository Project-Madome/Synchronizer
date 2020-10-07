use anyhow;
use async_trait::async_trait;
use reqwest;
use scraper::{Html, Selector};

use crate::models::{Metadata, MetadataBook};
use crate::parser::Parser;

pub struct Gallery {
    id: i32,
    request_data: Option<Box<String>>,
}

/// ```html
/// <!-- Response of https://hitomi.la/galleries/1744332.html -->
/// <!DOCTYPE html>
/// <html>
/// <head>
/// <meta charset="UTF-8">
/// <link rel="canonical" href="https://hitomi.la/doujinshi/kuro-no-ugomeku-rougoku-de-|-검은-꿈틀대는-감옥에서-한국어-1744332.html">
/// <meta http-equiv="refresh" content="0;url=https://hitomi.la/doujinshi/kuro-no-ugomeku-rougoku-de-|-검은-꿈틀대는-감옥에서-한국어-1744332.html">
/// <script type="text/javascript">
/// window.location.href = "https://hitomi.la/doujinshi/kuro-no-ugomeku-rougoku-de-|-검은-꿈틀대는-감옥에서-한국어-1744332.html"
/// </script>
/// <title>Redirect</title>
/// </head>
/// <body>
/// If you are not redirected automatically, follow the <a href="https://hitomi.la/doujinshi/kuro-no-ugomeku-rougoku-de-|-검은-꿈틀대는-감옥에서-한국어-1744332.html">link to the content</a>.
/// </body>
/// </html>
/// ```
impl Gallery {
    pub fn new(id: i32) -> Gallery {
        Gallery {
            id,
            request_data: None,
        }
    }

    pub fn is_nothing(&self, element: &scraper::ElementRef<'_>) -> bool {
        element.text().next().unwrap().trim() == "N/A"
    }

    pub fn parse_multiple_metadata(&self, element: scraper::ElementRef) -> Vec<String> {
        let ul_selector = Selector::parse("ul").unwrap();
        let li_selector = Selector::parse("li").unwrap();

        element
            .select(&ul_selector)
            .next()
            .unwrap()
            .select(&li_selector)
            .map(|element| element.text().next().unwrap().to_string())
            .collect::<Vec<_>>()
    }

    pub fn parse_characters(&self, element: scraper::ElementRef) -> Option<Vec<String>> {
        let characters = self.parse_multiple_metadata(element);

        if characters.is_empty() {
            return None;
        }

        Some(characters)
    }

    pub fn parse_groups(&self, element: scraper::ElementRef) -> Option<Vec<String>> {
        if self.is_nothing(&element) {
            return None;
        }

        let groups = self.parse_multiple_metadata(element);

        Some(groups)
    }

    pub fn parse_metadata(&self, document: &Html, metadata_type: Metadata) -> Metadata {
        let gallery_info_selector = Selector::parse(".gallery-info > table").unwrap();
        let tr_selector = Selector::parse("tr").unwrap();
        let td_selector = Selector::parse("td").unwrap();

        let r = document
            .select(&gallery_info_selector)
            .next()
            .unwrap()
            .select(&tr_selector)
            .find(|element| {
                let element = element.select(&td_selector).next().unwrap();

                element.text().next().unwrap() == metadata_type.as_str()
            })
            .unwrap()
            .select(&td_selector)
            .nth(1)
            .unwrap();

        match metadata_type {
            Metadata::Characters(_) => Metadata::Characters(self.parse_characters(r)),
            Metadata::Groups(_) => Metadata::Groups(self.parse_groups(r)),
            _ => metadata_type,
        }
    }
}

#[async_trait]
impl Parser for Gallery {
    type RequestData = String;
    type ParseData = MetadataBook;

    fn request_data(&self) -> anyhow::Result<&Box<Self::RequestData>> {
        match self.request_data {
            Some(ref rd) => Ok(rd),
            None => Err(anyhow::Error::msg("Can't get request_data")),
        }
    }

    async fn url(&self) -> anyhow::Result<String> {
        let gallery_url = format!("https://hitomi.la/galleries/{}.html", self.id);

        let client = reqwest::Client::builder().build()?;

        let gallery_html = client
            .get(gallery_url.as_str())
            .send()
            .await?
            .text()
            .await?;

        let document = Html::parse_document(gallery_html.as_str());
        let content_url_selector = Selector::parse("body > a").unwrap();

        let anchor_element = document.select(&content_url_selector).next().unwrap();

        let content_url = anchor_element
            .value()
            .attr("href")
            .expect("Can't find `Content URL` in `parser::Gallery`")
            .to_string();

        Ok(content_url)
    }

    async fn request(mut self) -> anyhow::Result<Box<Self>> {
        let content_url = self.url().await?;

        let client = reqwest::Client::builder().build()?;

        let content_html = client
            .get(content_url.as_str())
            .send()
            .await?
            .text()
            .await?;

        self.request_data = Some(Box::new(content_html));
        Ok(Box::new(self))
    }

    /// Groups
    /// Charcters
    async fn parse(&self) -> anyhow::Result<Self::ParseData> {
        let document = Html::parse_document(self.request_data()?.as_str());

        // let id = Metadata::ID(Some(self.id));
        let characters = (self.parse_metadata(&document, Metadata::Characters(None)));
        let groups = self.parse_metadata(&document, Metadata::Groups(None));

        let metadata_book = MetadataBook {
            characters,
            groups,
            id: Metadata::ID(None),
            title: Metadata::Title(None),
            artists: Metadata::Artists(None),
            series: Metadata::Series(None),
            tags: Metadata::Tags(None),
            language: Metadata::Language(None),
            content_type: Metadata::ContentType(None),
            created_at: Metadata::CreatedAt(None),
            thumbnail_url: Metadata::ThumbnailURL(None),
        };

        Ok(metadata_book)
    }
}

#[cfg(test)]
mod tests {
    use scraper::Html;

    use super::Gallery;
    use super::Metadata;
    use super::Parser;

    #[tokio::test]
    async fn parse_characters() -> anyhow::Result<()> {
        let gallery = Gallery::new(1277807);

        let gallery = gallery.request().await?;

        let document = Html::parse_document(gallery.request_data()?.as_str());

        let characters = gallery.parse_metadata(&document, Metadata::Characters(None));

        let expected = Metadata::Characters(Some(
            [
                "elf yamada",
                "haruhi suzumiya",
                "lum",
                "lyfa",
                "masamune izumi",
                "muramasa senju",
                "ranma saotome",
                "sagiri izumi",
                "shampoo",
                "shino asada",
                "suguha kirigaya",
            ]
            .iter()
            .map(|st| st.to_string())
            .collect::<Vec<_>>(),
        ));

        assert_eq!(expected, characters);

        Ok(())
    }

    #[tokio::test]
    async fn parse_characters_is_nothing() -> anyhow::Result<()> {
        let gallery = Gallery::new(1745756);

        let gallery = gallery.request().await?;

        let document = Html::parse_document(gallery.request_data()?.as_str());

        let characters = gallery.parse_metadata(&document, Metadata::Characters(None));

        let expected = Metadata::Characters(None);

        assert_eq!(expected, characters);

        Ok(())
    }

    #[tokio::test]
    async fn parse_groups() -> anyhow::Result<()> {
        let gallery = Gallery::new(1705277);

        let gallery = gallery.request().await?;

        let document = Html::parse_document(gallery.request_data()?.as_str());

        let groups = gallery.parse_metadata(&document, Metadata::Groups(None));

        let expected = Metadata::Groups(Some(vec!["haniya".to_string()]));

        assert_eq!(expected, groups);

        Ok(())
    }

    #[tokio::test]
    async fn parse_groups_is_nothing() -> anyhow::Result<()> {
        let gallery = Gallery::new(1454325);

        let gallery = gallery.request().await?;

        let document = Html::parse_document(gallery.request_data()?.as_str());

        let groups = gallery.parse_metadata(&document, Metadata::Groups(None));

        let expected = Metadata::Groups(None);

        assert_eq!(expected, groups);

        Ok(())
    }
}
