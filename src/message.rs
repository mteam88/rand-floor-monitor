use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use indoc::formatdoc;

use ethers::types::{H160, U256};

use ethers::prelude::LogMeta;

use crate::FragmentNftFilter;

#[derive(Clone, Debug, Default)]
pub(crate) struct Message {
    etherscan_link: String,
    collection_header: String,
    mu_token: MuToken,
    tokens: Vec<Token>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Token {
    token_id: U256,
    blur_link: String,
    flooring_link: String,
    opensea_pro_link: String,
    valuation: Option<Valuation>,
    top_bid: TopBid,
}

#[derive(Clone, Debug, Default)]
pub(crate)
struct Valuation {
    url: String,
    price: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate)
struct TopBid {
    url: String,
    kind: String,
    price: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate)
struct MuToken {
    dexscreener_link: String,
    name: String,
    derived_price: f64,
}


impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // create the message html that includes the information about the collection and the tokens
        let mut message = formatdoc!(
            r#"{0}
            {1}
            {2}

            "#,
            self.etherscan_link,
            self.collection_header,
            self.mu_token
        );

        for token in &self.tokens {
            let valuation = match &token.valuation {
                Some(valuation) => valuation.to_string(),
                None => {
                    "Error getting DeepNFTValue valuation for token".to_string()
                }
            };

            message.push_str(&formatdoc!(
                r#"
                Token {0}: <a href="{1}">Blur</a> -- <a href="{2}">Flooring</a> -- <a href="{3}">OpenSea Pro</a>
                {4}
                {5}
                Estimated Arbitrage Profit: {6} ETH

                "#,
                token.token_id,
                token.blur_link,
                token.flooring_link,
                token.opensea_pro_link,
                valuation,
                token.top_bid,
                token.top_bid.price - self.mu_token.derived_price
            ));
        }

        write!(f, "{}", message)?;

        Ok(())
    }
}

impl Display for Valuation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = formatdoc!(
            r#"DeepNFTValue valuation: <a href="{0}"> {1} ETH </a>"#,
            self.url,
            self.price,
        );

        write!(f, "{}", message)?;

        Ok(())
    }
}

impl Display for TopBid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = formatdoc!(
            r#"Top Bid (including fees): <a href={0}> {2} ETH on {1} </a>"#,
            self.url,
            self.kind,
            self.price,
        );

        write!(f, "{}", message)?;

        Ok(())
    }
}

impl Display for MuToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = formatdoc!(
            r#"{1} Derived Price: <a href="{0}"> {2} ETH </a>"#,
            self.dexscreener_link,
            self.name,
            self.derived_price,
        );

        write!(f, "{}", message)?;

        Ok(())
    }
}

impl Message {
    pub(crate) async fn fill_message(mut self, log: FragmentNftFilter, meta: LogMeta) -> Self {
        let tx_hash: String = format!("{:#x}", meta.transaction_hash);
        let collection_address: String = format!("{:#x}", log.collection);

        // create a link to the transaction on etherscan
        self.etherscan_link = format!("https://etherscan.io/tx/{tx_hash}");

        self.collection_header = match self.slug(&collection_address).await {
            Some(slug) => format! {"\nCollection: {}", slug},
            None => format! {"\nCollection: {collection_address}"},
        };

        self.mu_token =
            self.get_mu_token_details(&collection_address).await;

        // create links for each token id
        for token_id in log.token_ids {
            let token = Token {
                token_id,
                blur_link: format!("https://blur.io/asset/{collection_address}/{}", token_id),
                flooring_link: format!(
                    "https://www.flooring.io/nft-details/{collection_address}/{}",
                    token_id
                ),
                opensea_pro_link: format!(
                    "https://pro.opensea.io/nft/{collection_address}/{}",
                    token_id
                ),
                valuation: self.get_valuation(&collection_address, token_id).await,
                top_bid: self.get_top_bid(&collection_address, token_id).await,
            };

            self.tokens.push(token);
        }

        self
    }

    pub(crate) async fn get_mu_token_details(&self, collection: &str) -> MuToken {
        // use ethers RPC to call the `collectionInfo` function on the flooring contract for the given collection

        let client = crate::get_http_client().await;

        let flooring = crate::FlooringInterface::new(
            "0x8ad7892f15e6a3a1c0eecf83c30f414227434540"
                .parse::<H160>()
                .unwrap(),
            client.into(),
        );

        let collection_info = flooring
            .collection_info(collection.parse::<H160>().unwrap())
            .await
            .unwrap();

        let mu_token_address = collection_info.0;

        // now get the mu token price from moralis
        let client = reqwest::Client::new();

        let url = format! {"https://deep-index.moralis.io/api/v2.2/erc20/{:#x}/price?chain=eth", mu_token_address};

        let req = client
            .get(url)
            .header("accept", "application/json")
            .header("X-API-Key", dotenv::var("MORALIS_API_KEY").unwrap());

        let res = req.send().await.unwrap();

        // get json from response

        let json = res.json::<serde_json::Value>().await.unwrap();

        let mu_token_price = json["nativePrice"].as_object().unwrap()["value"]
            .as_str()
            .unwrap();

        let mu_token_price = mu_token_price.parse::<f64>().unwrap();

        let nft_derived_price = mu_token_price * 1_000_000_f64 / 10f64.powi(18);

        let mu_token_name = json["tokenName"].as_str().unwrap();

        let dexscreener_link = format!(
            "https://dexscreener.com/ethereum/{:#x}",
            mu_token_address
        );

        MuToken {
            dexscreener_link,
            name: mu_token_name.to_string(),
            derived_price: nft_derived_price,
        }
    }

    pub(crate) async fn get_top_bid(&self, collection: &str, token_id: U256) -> TopBid {
        let client = reqwest::Client::new();

        let url = format! {"https://api.reservoir.tools/orders/bids/v6?token={}%3A{}&status=active&normalizeRoyalties=true&sortBy=price&limit=1&displayCurrency=0x0000000000000000000000000000000000000000", collection, token_id};

        let req = client
            .get(url)
            .header("accept", "application/json")
            .header("x-api-key", dotenv::var("RESERVOIR_API_KEY").unwrap());

        let res = req.send().await.unwrap();

        // get json from response

        let json = res.json::<serde_json::Value>().await.unwrap();

        let top_bid = json["orders"][0]["price"]["netAmount"]["decimal"].to_string();

        let top_bid_url = json["orders"][0]["source"]["url"].to_string();

        let top_bid_kind = json["orders"][0]["source"]["name"].to_string();

        TopBid {
            url: top_bid_url,
            kind: top_bid_kind,
            price: top_bid.parse::<f64>().unwrap(),
        }
    }

    pub(crate) async fn get_valuation(&self, collection: &str, token_id: U256) -> Option<Valuation> {
        let details = match self.slug(collection).await {
            Some(slug) => {
                // use deepnftvalue api

                let client = reqwest::Client::new();

                let url = format! {"https://api.deepnftvalue.com/v1/tokens/{}/{}", slug, token_id};

                let req = client
                    .get(url)
                    .header(
                        reqwest::header::AUTHORIZATION,
                        dotenv::var("DEEP_API_KEY").unwrap(),
                    )
                    .header("accept", "application/json");

                let res = req.send().await.unwrap();

                // get json from response
                let json = res.json::<serde_json::Value>().await.unwrap();

                // if valuation is None, return after printing error
                let valuation = match json["valuation"].as_object() {
                    Some(valuation) => valuation,
                    None => {
                        println!("Error getting valuation: {:?}", json);
                        return None
                    }
                };

                // get valuation.price from json
                let price = valuation["price"].as_str().unwrap();

                // create link to deepnftvalue
                let url = format! {"https://deepnftvalue.com/asset/{}/{}", slug, token_id};

                return Some(Valuation {
                    url,
                    price: price.parse::<f64>().unwrap(),
                });
            }
            None => None,
        };

        details
    }

    pub(crate) async fn slug(&self, collection: &str) -> Option<String> {
        // hashmap of collection addresses to slugs
        let collection_slugs: HashMap<String, String> = {
            let mut inner = HashMap::new();
            // inner.insert(
            //     "0xb6a37b5d14d502c3ab0ae6f3a0e058bc9517786e".to_string(),
            //     "azukielementals".to_string(),
            // );
            inner.insert(
                "0xbd3531da5cf5857e7cfaa92426877b022e612cf8".to_string(),
                "pudgypenguins".to_string(),
            );
            inner.insert(
                "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d".to_string(),
                "boredapeyachtclub".to_string(),
            );
            inner.insert(
                "0xfd1b0b0dfa524e1fd42e7d51155a663c581bbd50".to_string(),
                "y00ts".to_string(),
            );
            inner.insert(
                "0xed5af388653567af2f388e6224dc7c4b3241c544".to_string(),
                "azuki".to_string(),
            );
            inner.insert(
                "0x8821bee2ba0df28761afff119d66390d594cd280".to_string(),
                "degods".to_string(),
            );
            inner.insert(
                "0x49cf6f5d44e70224e2e23fdcdd2c053f30ada28b".to_string(),
                "clonex".to_string(),
            );
            inner.insert(
                "0x60e4d786628fea6478f785a6d7e704777c86a7c6".to_string(),
                "mutant-ape-yacht-club".to_string(),
            );
            inner.insert(
                "0x8a90cab2b38dba80c64b7734e58ee1db38b8992e".to_string(),
                "doodles-official".to_string(),
            );
            inner.insert(
                "0x23581767a106ae21c074b2276d25e5c3e136a68b".to_string(),
                "proof-moonbirds".to_string(),
            );
            inner
        };

        collection_slugs
            .get(collection)
            .map(|slug| slug.to_string())
    }
}
