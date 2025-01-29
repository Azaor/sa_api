mod manager;
mod person;
mod repository;

pub use manager::PersonManager;
pub use person::Person;
pub use repository::{GetPeopleResponse, PersonRepository, PersonRepositoryError};
