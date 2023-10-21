use ethers::{
    contract::{abigen, Contract},
    core::types::ValueOrArray,
    providers::{Provider, StreamExt, Ws},
    prelude::LogMeta, types::U256
};
use std::{error::Error, sync::Arc, collections::HashMap};

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
    let event = Contract::event_of_type::<FragmentNftFilter>(client)
        .address(ValueOrArray::Array(vec![
            FLOORING.parse()?,
        ]));

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
    Provider::<Ws>::connect("wss://mainnet.infura.io/ws/v3/c60b0bb42f8a4c6481ecd229eddaca27")
        .await
        .unwrap()
}

async fn send_to_telegram(log: FragmentNftFilter, meta: LogMeta) {
    // create Bot
    let bot = Bot::new(dotenv::var("TELEGRAM_BOT_TOKEN").unwrap());
    // set parsemode to html
    let bot = bot.parse_mode(teloxide::types::ParseMode::Html);
    bot.send_message("@flooring_monitor".to_string(), get_log(log, meta).await).send().await.unwrap();
}

async fn get_log(log: FragmentNftFilter, meta: LogMeta) -> String {
    let mut out: String = "".to_string();
    // create a link to the transaction on etherscan
    let etherscan_link = format!("https://etherscan.io/tx/{:#x}", meta.transaction_hash);
    let etherscan_link = format!("<a href=\"{}\">{:#x}</a>", etherscan_link, meta.transaction_hash);
    out.push_str(&etherscan_link);

    // create links for each token id
    for token_id in log.token_ids {
        let blur_link = format!("https://blur.io/asset/{:#x}/{}", log.collection, token_id);
        let blur_link = format!("\n\n<a href=\"{}\">blur: {}</a>", blur_link, token_id);
        out.push_str(&blur_link);

        let flooring_link = format!("https://www.flooring.io/nft-details/{:#x}/{}", log.collection, token_id);
        let flooring_link = format!("\n<a href=\"{}\">flooring: {}</a>", flooring_link, token_id);
        out.push_str(&flooring_link);
        
        let opensea_pro_link = format!("https://pro.opensea.io/nft/{:#x}/{}", log.collection, token_id);
        let opensea_pro_link = format!("\n<a href=\"{}\">opensea pro: {}</a>", opensea_pro_link, token_id);
        out.push_str(&opensea_pro_link);

        let valuation = get_valuation(format!("{:#x}", log.collection), token_id).await;
        out.push_str(&format!("{}", valuation));
    }
    
    out
}

async fn get_valuation(collection: String, token_id: U256) -> String {
    // use deepnftvalue api 

    let client = reqwest::Client::new();

    let url = format!{"https://api.deepnftvalue.com/v1/tokens/{}/{}", slug(&collection).await, token_id};

    let req = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, "Token 6d3b85e2e7d3679c55dedc0f2b21ef2a72018061")
        .header("accept", "application/json");

    let res = req.send().await.unwrap();
        

    // get json from response
    let json = res.json::<serde_json::Value>().await.unwrap();
    let valuation = json["valuation"].as_object().unwrap();

    // get valuation.price from json
    let price = valuation["price"].as_str().unwrap();
    // get valuation.currency from json
    let currency = valuation["currency"].as_str().unwrap();

    let details_url = format!{"https://deepnftvalue.com/asset/{}/{}", slug(&collection).await, token_id};

    format!("\n<a href=\"{}\">DeepNFTValue: {} {}</a>", details_url, price, currency)
    
}

async fn slug(collection: &String) -> String {
    // hashmap of collection addresses to slugs
    let mut collection_slugs: HashMap<String, String> = HashMap::new();
    collection_slugs.insert("0xb6a37b5d14d502c3ab0ae6f3a0e058bc9517786e".to_string(), "azukielementals".to_string());
    collection_slugs.insert("0xbd3531da5cf5857e7cfaa92426877b022e612cf8".to_string(), "pudgypenguins".to_string());
    collection_slugs.insert("0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d".to_string(), "boredapeyachtclub".to_string());
    collection_slugs.insert("0xfd1b0b0dfa524e1fd42e7d51155a663c581bbd50".to_string(), "y00ts".to_string());

    collection_slugs.get(collection).unwrap().to_string()

}