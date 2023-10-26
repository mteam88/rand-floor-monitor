use ethers::{
    contract::{abigen, Contract},
    core::types::ValueOrArray,
    prelude::LogMeta,
    providers::{Http, Provider, StreamExt, Ws},
};
use teloxide::prelude::*;

use std::{error::Error, sync::Arc};

pub mod message;

abigen!(
    FlooringInterface,
    r#"[
        event FragmentNft(address indexed operator, address indexed onBehalfOf, address indexed collection, uint256[] tokenIds)
        function collectionInfo(address collection) external view returns (address fragmentToken, uint256 freeNftLength, uint64 lastUpdatedBucket, uint64 nextKeyId, uint64 activeSafeBoxCnt, uint64 infiniteCnt, uint64 nextActivityId)
    ]"#,
);

const FLOORING: &str = "0x3eb879cc9a0Ef4C6f1d870A40ae187768c278Da2";

/// Subscribe to a typed event stream without requiring a `Contract` instance.
/// In this example we subscribe Chainlink price feeds and filter out them
/// by address.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = get_wss_client().await;
    let client = Arc::new(client);

    // Build an Event by type. We are not tied to a contract instance. We use builder functions to
    // refine the event filter
    let mut event = Contract::event_of_type::<FragmentNftFilter>(client)
        .address(ValueOrArray::Array(vec![FLOORING.parse()?]));

    match dotenv::var("STARTING_BLOCK")
        .unwrap()
        .parse::<u64>()
        .unwrap()
    {
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

async fn get_wss_client() -> Provider<Ws> {
    Provider::<Ws>::connect(dotenv::var("WSS_RPC").unwrap())
        .await
        .unwrap()
}

async fn get_http_client() -> Provider<Http> {
    Provider::<Http>::try_from(dotenv::var("HTTP_RPC").unwrap().as_str())
        .expect("could not instantiate HTTP Provider")
}

async fn send_to_telegram(log: FragmentNftFilter, meta: LogMeta) {
    let msg = message::Message::default().fill_message(log, meta).await;
    println!("Total Profit: {}", msg.total_profit);

    if msg.total_profit <= dotenv::var("MINIMUM_PROFIT").unwrap().parse::<f64>().unwrap() {
        println!("Profit too low, not sending message");
        return;
    }

    // create Bot
    let bot = Bot::new(dotenv::var("TELEGRAM_BOT_TOKEN").unwrap());
    // set parsemode to html
    let bot = bot.parse_mode(teloxide::types::ParseMode::Html);
    match bot
        .send_message(
            "@flooring_monitor".to_string(),
            msg.to_string(),
        )
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
