use application::api::router::MainRouter;
use domain::{person::PersonManager, speech::manager::SpeechManager};
use dotenv::dotenv;
use infrastructure::{
    person::postgres::postgres_repository::PostgresPersonRepository,
    speech::postgres::repository::PostgresSpeechRepository,
};
use tokio::runtime::Runtime;

mod application;
mod domain;
mod infrastructure;
fn main() {
    dotenv().ok();
    // Check of env variables before starting the app.
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not found in env file");
    let _ = std::env::var("KEYCLOAK_CERTS_URL").expect("KEYCLOAK_CERTS_URL not found in env file");
    let database_timeout: u64 = std::env::var("DATABASE_TIMEOUT")
        .unwrap_or("100".to_string())
        .parse()
        .expect("DATABASE_TIMEOUT must be an u64");

    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let person_repository = PostgresPersonRepository::new(&db_url, database_timeout)
            .await
            .expect("Cannot connect to the DB");
        let speech_repository = PostgresSpeechRepository::new(&db_url, database_timeout)
            .await
            .expect("Cannot connect to the DB");
        let speech_manager = SpeechManager::new(Box::new(speech_repository));
        let person_manager = PersonManager::new(Box::new(person_repository));
        let main_router = MainRouter::new(person_manager, speech_manager);
        let _ = main_router.run().await.expect("An error occured");
    })
}
