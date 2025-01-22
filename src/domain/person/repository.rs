use super::person::Person;
use uuid::Uuid;

#[derive(Debug, PartialEq)]
pub enum PersonRepositoryError {
    PersonNotFound,
    PersonAlreadyExists,
    InternalError(String),
}

#[async_trait::async_trait]
pub trait PersonRepository: PersonClone + Send + Sync {
    async fn create_person(&self, person: &Person) -> Result<(), PersonRepositoryError>;
    async fn update_person(&self, person: &Person) -> Result<(), PersonRepositoryError>;
    async fn get_person_by_id(&self, uid: &Uuid) -> Result<Person, PersonRepositoryError>;
    async fn get_people(
        &self,
        page: u16,
        quantity: u16,
    ) -> Result<Vec<Person>, PersonRepositoryError>;
    async fn delete_person(&self, uid: &Uuid) -> Result<(), PersonRepositoryError>;
}
pub trait PersonClone {
    fn clone_box(&self) -> Box<dyn PersonRepository>;
}

impl<T> PersonClone for T
where
    T: 'static + PersonRepository + Clone,
{
    fn clone_box(&self) -> Box<dyn PersonRepository> {
        Box::new(self.clone())
    }
}

// We can now implement Clone manually by forwarding to clone_box.
impl Clone for Box<dyn PersonRepository> {
    fn clone(&self) -> Box<dyn PersonRepository> {
        self.clone_box()
    }
}
