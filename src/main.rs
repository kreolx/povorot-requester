use chrono::{DateTime, Timelike, Utc, TimeZone, Datelike};
use futures_lite::*;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use redis::{Commands, RedisError};
use serde::{Deserialize, Serialize};
use std::{env, thread, time, str};
use mongodb::bson::{doc};
use tokio::runtime::Runtime;


const REDIS_CON_STRING: &str = "REDIS_CON_STRING";
const RABBIT_CON_STRING: &str = "RABBIT_CON_STRING";
const MONGODB_CON_STRING: &str = "MONGODB_CON_STRING";
const MONGODB: &str = "povorotdb";
const MONGODB_REQUEST_COLLECTION: &str = "requests";
const COUNT_REQUEST: usize = 3;

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

        let _saved_queue = channel
            .queue_declare("request-notification", QueueDeclareOptions::default(), FieldTable::default());

        let mut consumer = channel
            .basic_consume(
                "save-requests",
                "rust_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("consume save request faild");

        let mongo_client = mongodb::Client::with_uri_str(mongodb_con_str)
            .await
            .expect("Faild to create MongoDb client");

        let rt = Runtime::new().unwrap();
        
        let _ = rt.spawn(async move {
            while let Some(delivery) = consumer.next().await {
                let (_, delivery) = delivery.expect("error in consumer");
                delivery.ack(BasicAckOptions::default())
                    .await
                    .expect("Ack");

                let text = str::from_utf8(&delivery.data).unwrap();
                let save_request: RequestSignupDto = serde_json::from_str(text).unwrap();
                let mut save_date = DateTime::parse_from_rfc3339(&save_request.date).unwrap();
                let minute = save_date.time().minute();
                if minute != 0 {
                    let dt = Utc.ymd(save_date.year(), save_date.month(), save_date.day())
                    .and_hms(save_date.hour(), 0, 0);
                    
                    save_date = DateTime::parse_from_rfc3339(&dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()).unwrap();
                }
                println!("{}", &save_date);
                let filter = doc! {"planed_at" : &save_date.format("%d.%m.%YT%H:%M:%S").to_string()};
                let request_collection = mongo_client
                    .database(MONGODB)
                    .collection::<RequestSignup>(MONGODB_REQUEST_COLLECTION)
                    .find(filter, None)
                    .await
                    .expect("Error get cursor");
                let c: usize  = request_collection.count().await;
                if c < COUNT_REQUEST {
                    let request = RequestSignup {
                        id: Option::Some(mongodb::bson::oid::ObjectId::new()),
                        car: save_request.car,
                        phone: save_request.phone,
                        description: save_request.description,
                        planed_at: save_date.format("%d.%m.%YT%H:%M:%S").to_string(),
                    };
                    let _ = mongo_client
                    .database(MONGODB)
                    .collection::<RequestSignup>(MONGODB_REQUEST_COLLECTION)
                    .insert_one(request, None).await;
                    let _confirm = channel
                        .basic_publish("", 
                        "request-notification", 
                        BasicPublishOptions::default(),
                                    delivery.data,
                                    BasicProperties::default(),)
                        .await
                        .expect("faild publish")
                        .await
                        .expect("faild confirm");
                } else {
                    let mut con = connect().unwrap();
                    
                    let _: usize = con.lrem(save_date.format("%d.%m.%Y").to_string(),
                    1,
                    &save_date.format("%d.%m.%YT%H-%M").to_string())
                    .unwrap();
                }
                println!("{:#?}", &c);
            }
        })
        .await;
    });
}

fn connect() -> Result<redis::Connection, RedisError> {
    let con_str = env::var(REDIS_CON_STRING).unwrap_or_else(|_| "redis://127.0.0.1/".into());
    let client = redis::Client::open(con_str)?;
    Ok(client.get_connection()?)
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
