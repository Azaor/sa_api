use uuid::Uuid;

#[derive(Clone)]
pub struct Sentence {
    uid: Uuid,
    speaker: Uuid,
    text: String,
    interrupted: bool,
}

impl Sentence {
    pub fn new(uid: &Uuid, speaker: &Uuid, text: &str, interrupted: bool) -> Self {
        Self {
            uid: uid.clone(),
            speaker: speaker.clone(),
            text: text.to_string(),
            interrupted,
        }
    }

    pub fn uid(&self) -> &Uuid {
        &self.uid
    }

    pub fn speaker(&self) -> &Uuid {
        &self.speaker
    }

    pub fn text(&self) -> &String {
        &self.text
    }

    pub fn interrupted(&self) -> bool {
        self.interrupted
    }
}
