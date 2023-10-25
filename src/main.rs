use ethers::{
    contract::{abigen, Contract},
    core::types::ValueOrArray,
    prelude::LogMeta,
    providers::{Provider, StreamExt, Ws},
    types::U256,
};
use std::{collections::HashMap, error::Error, sync::Arc};

use teloxide::prelude::*;

abigen!(
    FlooringInterface,
    r#"[
        event FragmentNft(address indexed operator, address indexed onBehalfOf, address indexed collection, uint256[] tokenIds)
    ]"#,
);

const FLOORING: &str = "0x3eb879cc9a0Ef4C6f1d870A40ae187768c278Da2";

/// Subscribe to a typed event stream without requiring a `Contract` instance.
/// In this example we subscribe Chainlink price feeds and filter out them
/// by address.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = get_client().await;
    let client = Arc::new(client);

    // Build an Event by type. We are not tied to a contract instance. We use builder functions to
    // refine the event filter
    let mut event = Contract::event_of_type::<FragmentNftFilter>(client)
        .address(ValueOrArray::Array(vec![FLOORING.parse()?]));

    match dotenv::var("STARTING_BLOCK").unwrap().parse::<u64>().unwrap() {
        0 => {
            println!("Starting from latest block");
        }
        block => {
            println!("Starting from block {}", block);
            event = event.from_block(block);
        }
    }

    let mut stream = event.subscribe_with_meta().await?;

    // Note that `log` has type FragmentNftUpdateFilter
    while let Some(Ok((log, meta))) = stream.next().await {
        // send the log to telegram
        println!("log: {:?}", log);
        println!("meta: {:?}", meta);

        send_to_telegram(log, meta).await;
    }

    Ok(())
}

async fn get_client() -> Provider<Ws> {
    Provider::<Ws>::connect(dotenv::var("RPC").unwrap())
        .await
        .unwrap()
}

async fn send_to_telegram(log: FragmentNftFilter, meta: LogMeta) {
    // create Bot
    let bot = Bot::new(dotenv::var("TELEGRAM_BOT_TOKEN").unwrap());
    // set parsemode to html
    let bot = bot.parse_mode(teloxide::types::ParseMode::Html);
    match bot
        .send_message("@flooring_monitor".to_string(), get_log(log, meta).await)
        .send()
        .await
    {
        Ok(_) => println!("Message sent"),
        Err(e) => {
            println!("Error sending message: {:?}", e);
            // sleep for 35 seconds to avoid spamming telegram
            tokio::time::sleep(tokio::time::Duration::from_secs(35)).await;
        }
    }
}

async fn get_log(log: FragmentNftFilter, meta: LogMeta) -> String {
    let mut out: String = "".to_string();
    // create a link to the transaction on etherscan
    let etherscan_link = format!("https://etherscan.io/tx/{:#x}", meta.transaction_hash);
    let etherscan_link = format!(
        "<a href=\"{}\">{:#x}</a>",
        etherscan_link, meta.transaction_hash
    );
    out.push_str(&etherscan_link);

    let collection_name = match slug(&format!("{:#x}", log.collection)).await {
        Some(slug) => format! {"\nCollection: {}", slug},
        None => format! {"\nCollection: {:#x}", log.collection},
    };
    out.push_str(&collection_name);

    // create links for each token id
    for token_id in log.token_ids {
        let blur_link = format!("https://blur.io/asset/{:#x}/{}", log.collection, token_id);
        let blur_link = format!("\n\n<a href=\"{}\">blur: {}</a>", blur_link, token_id,);
        out.push_str(&blur_link);

        let flooring_link = format!(
            "https://www.flooring.io/nft-details/{:#x}/{}",
            log.collection, token_id
        );
        let flooring_link = format!("\n<a href=\"{}\">flooring: {}</a>", flooring_link, token_id);
        out.push_str(&flooring_link);

        let opensea_pro_link = format!(
            "https://pro.opensea.io/nft/{:#x}/{}",
            log.collection, token_id
        );
        let opensea_pro_link = format!(
            "\n<a href=\"{}\">opensea pro: {}</a>",
            opensea_pro_link, token_id
        );
        out.push_str(&opensea_pro_link);

        let valuation = get_valuation(format!("{:#x}", log.collection), token_id).await;
        out.push_str(valuation.as_str());

        let top_bid = get_top_bid(format!("{:#x}", log.collection), token_id).await;
        out.push_str(&format!("{}", top_bid));
    }

    out
}

async fn get_top_bid(collection: String, token_id: U256) -> String {
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

    let out = format!(
        "\n<a href={}>Top Bid: {} ETH on {}</a>",
        top_bid_url, top_bid, top_bid_kind
    );

    out
}

async fn get_valuation(collection: String, token_id: U256) -> String {
    let details = match slug(&collection).await {
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
                    return "\nError getting valuation ):".to_string();
                }
            };

            // get valuation.price from json
            let price = valuation["price"].as_str().unwrap();
            // get valuation.currency from json
            let currency = valuation["currency"].as_str().unwrap();

            let url = format! {"https://api.deepnftvalue.com/v1/collections/{}", slug};

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
            let floor = match json["floor_price"].as_str() {
                Some(floor) => floor,
                None => {
                    println!("Error getting floor: {:?}", json);
                    return "\nError getting floor ):".to_string();
                }
            };

            // create link to deepnftvalue
            let url = format! {"https://deepnftvalue.com/asset/{}/{}", slug, token_id};

            format!(
                "\n<a href=\"{}\">DeepNFTValue: {} {}; floor: {}</a>",
                url, price, currency, floor
            )
        }
        None => "\nCollection is not on DeepNFTValue ):".to_string(),
    };

    details
}

async fn slug(collection: &String) -> Option<String> {
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
