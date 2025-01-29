use super::{
    person::Person,
    repository::{GetPeopleResponse, PersonRepository, PersonRepositoryError},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PersonManager {
    repository: Box<dyn PersonRepository>,
}

impl PersonManager {
    pub fn new(repository: Box<dyn PersonRepository>) -> Self {
        return PersonManager { repository };
    }

    pub async fn create_person(&self, person: Person) -> Result<(), PersonRepositoryError> {
        self.repository.create_person(&person).await
    }

    pub async fn _update_person(&self, person: Person) -> Result<(), PersonRepositoryError> {
        self.repository.update_person(&person).await
    }

    pub async fn get_person_by_id(&self, uid: &Uuid) -> Result<Person, PersonRepositoryError> {
        self.repository.get_person_by_id(uid).await
    }

    pub async fn get_people(
        &self,
        page: u16,
        quantity: u16,
    ) -> Result<GetPeopleResponse, PersonRepositoryError> {
        self.repository.get_people(page, quantity).await
    }

    pub async fn delete_person(&self, uid: &Uuid) -> Result<(), PersonRepositoryError> {
        self.repository.delete_person(uid).await
    }
}
