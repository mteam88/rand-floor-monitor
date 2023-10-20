use ethers::{
    contract::{abigen, Contract},
    core::types::ValueOrArray,
    providers::{Provider, StreamExt, Ws},
    prelude::LogMeta
};
use std::{error::Error, sync::Arc};

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
    bot.send_message("@flooring_monitor".to_string(), get_log(log, meta)).send().await.unwrap();
}

fn get_log(log: FragmentNftFilter, meta: LogMeta) -> String {
    let mut out: String = "".to_string();
    // create a link to the transaction on etherscan
    let etherscan_link = format!("https://etherscan.io/tx/{:#x}", meta.transaction_hash);
    let etherscan_link = format!("<a href=\"{}\">{:#x}</a>", etherscan_link, meta.transaction_hash);
    out.push_str(&etherscan_link);

    // create links for each token id
    for token_id in log.token_ids {
        let blur_link = format!("https://blur.io/asset/{:#x}/{}", log.collection, token_id);
        let blur_link = format!("\n<a href=\"{}\">blur: {}</a>", blur_link, token_id);
        out.push_str(&blur_link);

        let flooring_link = format!("https://www.flooring.io/nft-details/{:#x}/{}", log.collection, token_id);
        let flooring_link = format!("\n<a href=\"{}\">flooring: {}</a>", flooring_link, token_id);
        out.push_str(&flooring_link);
        
        let opensea_pro_link = format!("https://pro.opensea.io/nft/{:#x}/{}", log.collection, token_id);
        let opensea_pro_link = format!("\n<a href=\"{}\">opensea pro: {}</a>", opensea_pro_link, token_id);
        out.push_str(&opensea_pro_link);
    }
    
    out
}
