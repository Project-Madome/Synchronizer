use std::convert::TryInto;

use anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest;

use crate::models::Language;

use super::Parser;

/// # Nozomi Parser
/// Not needed VPN for Nozomi Parser
///
/// ##
pub struct Nozomi {
    page: usize,
    per_page: usize,
    language: String,
    request_data: Option<Box<Bytes>>,
}

impl Nozomi {
    pub fn new(page: usize, per_page: usize, language: Language) -> Nozomi {
        Nozomi {
            page,
            per_page,
            language: language.into(),
            request_data: None,
        }
    }
}

#[async_trait]
impl Parser for Nozomi {
    type RequestData = Bytes;
    type ParseData = Vec<i32>;

    fn request_data(&self) -> anyhow::Result<&Box<Self::RequestData>> {
        match self.request_data {
            Some(ref rd) => Ok(rd),
            None => Err(anyhow::Error::msg("Can't get request_data")),
        }
    }

    async fn url(&self) -> anyhow::Result<String> {
        Ok(format!(
            "https://ltn.hitomi.la/index-{}.nozomi",
            self.language.to_lowercase()
        ))
    }

    async fn request(mut self) -> anyhow::Result<Box<Self>> {
        let client = reqwest::Client::builder().build()?;

        let start_bytes = (self.page - 1) * self.per_page * 4;
        let end_bytes = start_bytes + self.per_page * 4 - 1;

        let bytes = client
            .get(self.url().await?.as_str())
            .header("Range", format!("bytes={}-{}", start_bytes, end_bytes))
            .send()
            .await?
            .bytes()
            .await?;

        self.request_data = Some(Box::new(bytes));
        Ok(Box::new(self))
    }

    async fn parse(&self) -> anyhow::Result<Self::ParseData> {
        let request_data = self.request_data()?;

        let mut res = vec![];

        'a: for i in (0..request_data.len()).step_by(4) {
            let mut temp: i32 = 0;

            for j in 0..3 {
                // https://github.com/Project-Madome/Madome-Synchronizer/issues/1
                // temp += TryInto::<i32>::try_into(request_data[i + (3 - j)])? << (j << 3);
                if let Some(a) = request_data.get(i + (3 - j)) {
                    temp += TryInto::<i32>::try_into(*a)? << (j << 3);
                } else {
                    break 'a;
                }
            }

            res.push(temp);
        }

        res.sort_by(|a, b| b.cmp(a));

        Ok(res)
    }
}

#[cfg(test)]
mod test {
    use crate::models::Language;

    use super::Nozomi;
    use super::Parser;

    #[tokio::test]
    async fn parse_nozomi() -> anyhow::Result<()> {
        let nozomi_parser = Nozomi::new(1, 25, Language::Korean);

        let nozomi_parser = nozomi_parser.request().await?;

        let pd = nozomi_parser.parse().await?;

        assert_eq!(25, pd.len());

        Ok(())
    }

    #[tokio::test]
    async fn parse_nozomi_index_out_of_bounds() -> anyhow::Result<()> {
        let nozomi_parser = Nozomi::new(20, 1000000, Language::Korean);

        let nozomi_parser = nozomi_parser.request().await?;
        let pd = nozomi_parser.parse().await?;

        Ok(())
    }
}
