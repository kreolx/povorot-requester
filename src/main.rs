use chrono::DateTime;
use futures_lite::*;
use hyper::body::Buf;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use mongodb::bson::de::Result;
use mongodb::error::Error;
use redis::{Commands, RedisError};
use serde::{Deserialize, Serialize};
use std::{env, thread, time, str, vec};
use mongodb::{Client, Collection};
use mongodb::bson::{doc};


const REDIS_CON_STRING: &str = "REDIS_CON_STRING";
const RABBIT_CON_STRING: &str = "RABBIT_CON_STRING";
const MONGODB_CON_STRING: &str = "MONGODB_CON_STRING";
const MONGODB: &str = "povorotdb";
const MONGODB_REQUEST_COLLECTION: &str = "requests";

#[tokio::main]
async fn main() {
    let rabbit_con_str =
        env::var(RABBIT_CON_STRING).unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".into());
    let mongodb_con_str = env::var(MONGODB_CON_STRING).unwrap_or_else(|_| "mongodb://127.0.0.1".into());
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

        let mongo_client = mongodb::Client::with_uri_str(mongodb_con_str)
            .await
            .expect("Faild to create MongoDb client");
        
        async_global_executor::spawn(async move {
            while let Some(delivery) = consumer.next().await {
                let (_, delivery) = delivery.expect("error in consumer");
                delivery.ack(BasicAckOptions::default())
                    .await
                    .expect("Ack");

                let text = str::from_utf8(&delivery.data).unwrap();
                let save_request: RequestSignupDto = serde_json::from_str(text).unwrap();
                let filter = doc! {"planed_at" : &save_request.date};
                let request_collection = mongo_client
                    .database(MONGODB)
                    .collection::<RequestSignup>(MONGODB_REQUEST_COLLECTION)
                    .find(filter, None)
                    .await
                    .expect("Error get cursor");
                let c: usize  = request_collection.count().await;
                if c < 3 {
                    let request = RequestSignup {
                        id: Option::Some(mongodb::bson::oid::ObjectId::new()),
                        car: save_request.car,
                        phone: save_request.phone,
                        description: save_request.description,
                        planed_at: save_request.date,
                    };
                    let _ = mongo_client
                    .database(MONGODB)
                    .collection::<RequestSignup>(MONGODB_REQUEST_COLLECTION)
                    .insert_one(request, None).await;
                }
                println!("{:#?}", &c);
            }
        })
        .detach();
        loop {
            thread::sleep(time::Duration::from_millis(1));
        }
    });
}

#[derive(Debug, Deserialize, Serialize)]
struct  RequestSignupDto {
    date: String,
    phone: String,
    car: String,
    description: String,
}

#[derive(Deserialize, Serialize)]
struct RequestSignup {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<mongodb::bson::oid::ObjectId>,
    planed_at: String,
    phone: String,
    car: String,
    description: String,
} 
