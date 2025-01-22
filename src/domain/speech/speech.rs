use std::fmt::Display;

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone)]
pub enum SpeechStatus {
    Pending,
    Validated,
}

impl TryFrom<&str> for SpeechStatus {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "PENDING" => Self::Pending,
            "VALIDATED" => Self::Validated,
            _ => return Err("Unexpected speech status value".to_owned()),
        })
    }
}

impl Display for SpeechStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeechStatus::Pending => f.write_str("PENDING"),
            SpeechStatus::Validated => f.write_str("VALIDATED"),
        }
    }
}

use super::sentence::Sentence;
pub struct Speech {
    uid: Uuid,
    name: String,
    date: DateTime<Utc>,
    speakers: Vec<Uuid>,
    sentences: Vec<Sentence>,
    media: String,
    speech_status: SpeechStatus,
}

impl Speech {
    pub fn new(
        uid: &Uuid,
        name: &str,
        date: DateTime<Utc>,
        speakers: &[Uuid],
        sentences: &[Sentence],
        media: &str,
        speech_status: SpeechStatus,
    ) -> Self {
        return Speech {
            uid: uid.clone(),
            name: name.to_string(),
            date: date,
            speakers: speakers.to_vec(),
            sentences: sentences.to_vec(),
            media: media.to_string(),
            speech_status,
        };
    }

    pub fn uid(&self) -> &Uuid {
        &self.uid
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn date(&self) -> &DateTime<Utc> {
        &self.date
    }

    pub fn speakers(&self) -> &Vec<Uuid> {
        &self.speakers
    }

    pub fn update_speakers(&mut self, speakers: &[Uuid]) {
        self.speakers = speakers.to_vec();
    }

    pub fn sentences(&self) -> &Vec<Sentence> {
        &self.sentences
    }

    pub fn media(&self) -> &String {
        &self.media
    }

    pub fn speech_status(&self) -> &SpeechStatus {
        &self.speech_status
    }
}
