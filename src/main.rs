use chrono::DateTime;
use futures_lite::*;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use redis::{Commands, RedisError};
use serde::{Deserialize, Serialize};
use std::{env, thread, time, str};


const REDIS_CON_STRING: &str = "REDIS_CON_STRING";
const RABBIT_CON_STRING: &str = "RABBIT_CON_STRING";

#[tokio::main]
async fn main() {
    let rabbit_con_str =
        env::var(RABBIT_CON_STRING).unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".into());
    async_global_executor::block_on(async {
        let conn = Connection::connect(
            &rabbit_con_str,
            ConnectionProperties::default().with_default_executor(8),
        )
        .await
        .unwrap();
        let channel = conn.create_channel().await.unwrap();
        let _queue = channel
            .queue_declare(
                "save-requests",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .unwrap();
        let mut consumer = channel
            .basic_consume(
                "save-requests",
                "rust_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("basic consume");
        
        async_global_executor::spawn(async move {
            while let Some(delivery) = consumer.next().await {
                let (_, delivery) = delivery.expect("error in consumer");
                delivery.ack(BasicAckOptions::default()).await.expect("Ack");
                let text = str::from_utf8(&delivery.data).unwrap();
                let save_request: SaveRequest = serde_json::from_str(text).unwrap();
                println!("{:#?}", save_request);
            }
        })
        .detach();
        loop {
            thread::sleep(time::Duration::from_millis(1));
        }
    });
}

#[derive(Debug, Deserialize, Serialize)]
struct  SaveRequest {
    date: String,
    phone: String,
    car: String,
    description: String,
}
