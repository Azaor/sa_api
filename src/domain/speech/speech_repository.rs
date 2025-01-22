use uuid::Uuid;

use crate::domain::person::PersonRepositoryError;

use super::speech::Speech;

#[derive(Debug, PartialEq)]
pub enum SpeechRepositoryError {
    PersonError(PersonRepositoryError),
    SpeechNotFound,
    SpeechAlreadyExists,
    InternalError(String),
}

#[async_trait::async_trait]
pub trait SpeechRepository: SpeechClone + Send + Sync {
    async fn create_speech(&self, speech: &Speech) -> Result<(), SpeechRepositoryError>;
    async fn get_speech_by_id(&self, uid: Uuid) -> Result<Speech, SpeechRepositoryError>;
    async fn get_speech(
        &self,
        page: u16,
        quantity: u16,
        speakers: &[Uuid],
    ) -> Result<Vec<Speech>, SpeechRepositoryError>;
    async fn delete_speech(&self, uid: Uuid) -> Result<(), SpeechRepositoryError>;
}

pub trait SpeechClone {
    fn clone_box(&self) -> Box<dyn SpeechRepository>;
}

impl<T> SpeechClone for T
where
    T: 'static + SpeechRepository + Clone,
{
    fn clone_box(&self) -> Box<dyn SpeechRepository> {
        Box::new(self.clone())
    }
}

// We can now implement Clone manually by forwarding to clone_box.
impl Clone for Box<dyn SpeechRepository> {
    fn clone(&self) -> Box<dyn SpeechRepository> {
        self.clone_box()
    }
}
