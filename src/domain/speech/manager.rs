use uuid::Uuid;

use super::{
    speech_repository::{SpeechRepository, SpeechRepositoryError},
    Speech,
};

#[derive(Clone)]
pub struct SpeechManager {
    repository: Box<dyn SpeechRepository>,
}

impl SpeechManager {
    pub fn new(repository: Box<dyn SpeechRepository>) -> Self {
        return SpeechManager { repository };
    }

    pub async fn create_speech(&self, speech: Speech) -> Result<(), SpeechRepositoryError> {
        self.repository.create_speech(&speech).await
    }

    pub async fn get_speech_by_id(&self, uid: Uuid) -> Result<Speech, SpeechRepositoryError> {
        self.repository.get_speech_by_id(uid).await
    }

    pub async fn get_speech(
        &self,
        page: u16,
        quantity: u16,
        speakers: &[Uuid],
    ) -> Result<Vec<Speech>, SpeechRepositoryError> {
        self.repository.get_speech(page, quantity, speakers).await
    }

    pub async fn delete_speech(&self, uid: Uuid) -> Result<(), SpeechRepositoryError> {
        self.repository.delete_speech(uid).await
    }
}
