use std::{collections::HashMap, str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, Error, PgPool, Row};
use tokio::time;
use uuid::Uuid;

use crate::domain::{
    self,
    person::PersonRepositoryError,
    speech::{
        sentence::Sentence,
        speech_repository::{SpeechRepository, SpeechRepositoryError},
        Speech,
    },
};

impl From<Error> for SpeechRepositoryError {
    fn from(value: Error) -> Self {
        match value {
            Error::Database(database_error) => {
                if database_error.is_unique_violation() || database_error.is_check_violation() {
                    return Self::SpeechAlreadyExists;
                }
                if database_error.is_foreign_key_violation() {
                    return Self::PersonError(PersonRepositoryError::PersonNotFound);
                }
                return Self::InternalError(database_error.to_string());
            }
            Error::RowNotFound => {
                return Self::SpeechNotFound;
            }
            _ => return Self::InternalError(value.to_string()),
        }
    }
}

impl TryFrom<PgRow> for Sentence {
    type Error = SpeechRepositoryError;

    fn try_from(value: PgRow) -> Result<Self, Self::Error> {
        let uid: &str = value.try_get("uid")?;
        let speaker: &str = value.try_get("speaker")?;
        let text: &str = value.try_get("text")?;
        let interrupted: bool = value.try_get("interrupted")?;
        return Ok(Self::new(
            &Uuid::from_str(uid)
                .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))?,
            &Uuid::from_str(speaker)
                .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))?,
            text,
            interrupted,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct PostgresSpeechRepository {
    url: String,
    timeout: u64,
}

async fn init_table_async(url: &str, timeout: u64) -> Result<(), SpeechRepositoryError> {
    let connection = time::timeout(Duration::from_millis(timeout), PgPool::connect(url))
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
    let create_speech_table_query = r#"CREATE TABLE IF NOT EXISTS speech (
        uid CHAR(36) PRIMARY KEY,
        name VARCHAR,
        date TIMESTAMPTZ,
        media VARCHAR,
        status VARCHAR,
        CONSTRAINT unique_speech UNIQUE (name, date, media)
    )"#;
    let _result = time::timeout(
        Duration::from_millis(timeout),
        sqlx::query(&create_speech_table_query).execute(&connection),
    )
    .await
    .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
    let create_speech_table_query = r#"CREATE TABLE IF NOT EXISTS sentence (
        uid CHAR(36) PRIMARY KEY,
        speech_uid CHAR(36),
        speaker CHAR(36),
        text VARCHAR,
        interrupted BOOLEAN,
        index INT,
        CONSTRAINT FK_SentenceSpeech FOREIGN KEY (speech_uid) REFERENCES speech(uid),
        CONSTRAINT FK_SentencePerson FOREIGN KEY (speaker) REFERENCES person(uid)
    )"#;
    let _result = time::timeout(
        Duration::from_millis(timeout),
        sqlx::query(&create_speech_table_query).execute(&connection),
    )
    .await
    .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
    let create_speech_person_table_query = r#"CREATE TABLE IF NOT EXISTS speech_person (
        speech_uid CHAR(36),
        speaker CHAR(36),
        CONSTRAINT FK_SentenceSpeech FOREIGN KEY (speech_uid) REFERENCES speech(uid),
        CONSTRAINT FK_SentencePerson FOREIGN KEY (speaker) REFERENCES person(uid)
    )"#;
    let _result = time::timeout(
        Duration::from_millis(timeout),
        sqlx::query(&create_speech_person_table_query).execute(&connection),
    )
    .await
    .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
    Ok(())
}

impl PostgresSpeechRepository {
    pub async fn new(url: &str, timeout: u64) -> Result<Self, SpeechRepositoryError> {
        init_table_async(url, timeout).await?;
        Ok(Self {
            url: url.to_string(),
            timeout: timeout,
        })
    }
}

#[async_trait::async_trait]
impl SpeechRepository for PostgresSpeechRepository {
    async fn create_speech(
        &self,
        speech: &domain::speech::Speech,
    ) -> Result<(), SpeechRepositoryError> {
        let connection = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;

        let mut tx = connection.begin().await?;
        let create_speech_query = format!(
            "INSERT INTO speech VALUES ('{}', '{}', '{}', '{}', '{}');",
            speech.uid(),
            speech.name(),
            speech.date().to_rfc3339(),
            speech.media(),
            speech.speech_status()
        );
        let result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query(&create_speech_query).execute(&mut *tx),
        )
        .await;
        if result.is_err() {
            tx.rollback().await?;
            return Err(result
                .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))
                .unwrap_err());
        }
        let result = result.unwrap();
        if result.is_err() {
            tx.rollback().await?;
            return Err(result.map_err(|e| e.into()).unwrap_err());
        }
        for speaker in speech.speakers() {
            let result = time::timeout(
                Duration::from_millis(self.timeout),
                sqlx::query("INSERT INTO speech_person VALUES ($1, $2);")
                    .bind(speech.uid().to_string())
                    .bind(speaker.to_string())
                    .execute(&mut *tx),
            )
            .await;
            if result.is_err() {
                tx.rollback().await?;
                return Err(result
                    .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))
                    .unwrap_err());
            }
            let result = result.unwrap();
            if result.is_err() {
                tx.rollback().await?;
                return Err(result.map_err(|e| e.into()).unwrap_err());
            }
        }
        for (idx, sentence) in speech.sentences().iter().enumerate() {
            let result = time::timeout(
                Duration::from_millis(self.timeout),
                sqlx::query("INSERT INTO sentence VALUES ($1, $2, $3, $4, $5, $6)")
                    .bind(sentence.uid().to_string())
                    .bind(speech.uid().to_string())
                    .bind(sentence.speaker().to_string())
                    .bind(sentence.text())
                    .bind(sentence.interrupted())
                    .bind(idx as i64)
                    .execute(&mut *tx),
            )
            .await;
            if result.is_err() {
                tx.rollback().await?;
                return Err(result
                    .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))
                    .unwrap_err());
            }
            let result = result.unwrap();
            if result.is_err() {
                tx.rollback().await?;
                return Err(result.map_err(|e| e.into()).unwrap_err());
            }
        }
        tx.commit().await?;
        return Ok(());
    }

    async fn get_speech_by_id(&self, uid: Uuid) -> Result<Speech, SpeechRepositoryError> {
        let connection = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;

        let speech_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT uid, name, date, media, status FROM speech WHERE uid = $1;")
                .bind(uid.to_string())
                .fetch_one(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let sentences_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT uid, speech_uid, speaker, text, interrupted, index, status FROM sentence WHERE speech_uid = $1 ORDER BY index;").bind(uid.to_string()).fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let mut sentences = Vec::new();
        for sentence in sentences_result {
            sentences.push(Sentence::try_from(sentence)?);
        }

        let speech_person_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT speech_uid, speaker FROM speech_person WHERE speech_uid = $1;")
                .bind(uid.to_string())
                .fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let mut speakers = Vec::new();
        for speech_person in speech_person_result {
            let speaker: &str = speech_person.get("speaker");
            speakers.push(
                Uuid::from_str(speaker)
                    .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))?,
            );
        }
        let speech_uid: &str = speech_result.get("uid");
        let name: &str = speech_result.get("name");
        let date: DateTime<Utc> = speech_result.get("date");
        let media: &str = speech_result.get("media");
        let status: &str = speech_result.get("status");
        return Ok(Speech::new(
            &Uuid::from_str(speech_uid)
                .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))?,
            name,
            date,
            &speakers,
            &sentences,
            media,
            status
                .try_into()
                .map_err(|e| SpeechRepositoryError::InternalError(e))?,
        ));
    }
    async fn delete_speech(&self, uid: Uuid) -> Result<(), SpeechRepositoryError> {
        let connection = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let mut tx = connection.begin().await?;
        let speech_person_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("DELETE FROM speech_person WHERE speech_uid = $1;")
                .bind(uid.to_string())
                .execute(&mut *tx),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()));
        if speech_person_result.is_err() {
            tx.rollback().await?;
            return Err(SpeechRepositoryError::InternalError(
                "Cannot delete speech from db".to_string(),
            ));
        }
        let speech_person_result = speech_person_result.unwrap();
        if speech_person_result.is_err() {
            tx.rollback().await?;
            return Err(speech_person_result.map_err(|e| e.into()).unwrap_err());
        }
        let sentences_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("DELETE FROM sentence WHERE speech_uid = $1;")
                .bind(uid.to_string())
                .execute(&mut *tx),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()));
        if sentences_result.is_err() {
            tx.rollback().await?;
            return Err(SpeechRepositoryError::InternalError(
                "Cannot delete speech from db".to_string(),
            ));
        }
        let sentences_result = sentences_result.unwrap();
        if sentences_result.is_err() {
            tx.rollback().await?;
            return Err(sentences_result.map_err(|e| e.into()).unwrap_err());
        }
        let speech_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("DELETE FROM speech WHERE uid = $1;")
                .bind(uid.to_string())
                .execute(&mut *tx),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()));
        if speech_result.is_err() {
            tx.rollback().await?;
            return Err(SpeechRepositoryError::InternalError(
                "Cannot delete speech from db".to_string(),
            ));
        }
        let speech_result = speech_result.unwrap();
        if speech_result.is_err() {
            tx.rollback().await?;
            return Err(speech_result.map_err(|e| e.into()).unwrap_err());
        }
        tx.commit().await?;
        Ok(())
    }
    async fn get_speech(
        &self,
        page: u16,
        quantity: u16,
        speakers: &[Uuid],
    ) -> Result<Vec<Speech>, SpeechRepositoryError> {
        if speakers.is_empty() {
            self.get_all_speech(page, quantity).await
        } else {
            self.get_speech_by_speakers_id(page, quantity, &speakers)
                .await
        }
    }
}

impl PostgresSpeechRepository {
    async fn get_speech_by_speakers_id(
        &self,
        page: u16,
        quantity: u16,
        speakers_id: &[Uuid],
    ) -> Result<Vec<Speech>, SpeechRepositoryError> {
        let connection = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;

        let list_speakers_id = speakers_id
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<String>>();
        let speech_person_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query(
                "SELECT speech_uid FROM speech_person WHERE speaker = ANY($1) LIMIT $2 OFFSET $3;",
            )
            .bind(list_speakers_id)
            .bind(quantity as i32)
            .bind((page * quantity) as i32)
            .fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let mut speech_uids = Vec::new();
        for speech_person in speech_person_result {
            let speech_uid: &str = speech_person.get("speech_uid");
            speech_uids.push(speech_uid.to_string());
        }
        let list_uid = speech_uids
            .iter()
            .map(|speech_uid| speech_uid.to_string())
            .collect::<Vec<String>>();

        let speech_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT uid, name, date, media, status FROM speech WHERE uid = ANY($1);")
                .bind(list_uid)
                .fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let mut speechs = HashMap::new();
        for speech in speech_result {
            let speech_uid: &str = speech.get("uid");
            let name: &str = speech.get("name");
            let date: DateTime<Utc> = speech.get("date");
            let media: &str = speech.get("media");
            let status: &str = speech.get("status");
            speechs.insert(
                speech_uid.to_string(),
                Speech::new(
                    &Uuid::from_str(&speech_uid)
                        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))?,
                    name,
                    date,
                    &[],
                    &[],
                    media,
                    status
                        .try_into()
                        .map_err(|e| SpeechRepositoryError::InternalError(e))?,
                ),
            );
        }
        let speech_list = speechs
            .keys()
            .map(|speaker| speaker.to_string())
            .collect::<Vec<String>>();

        let speech_person_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query(
                "SELECT speech_uid, speaker FROM speech_person WHERE speech_uid = ANY($1);",
            )
            .bind(speech_list)
            .fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;

        let mut speakers = HashMap::new();
        for speech_person in speech_person_result {
            let uid: &str = speech_person.get("speech_uid");
            let speaker: &str = speech_person.get("speaker");
            speakers
                .entry(uid.to_string())
                .and_modify(|val: &mut Vec<Uuid>| {
                    val.push(Uuid::from_str(speaker).expect("uid format expected"))
                })
                .or_insert(vec![Uuid::from_str(speaker).expect("uid format expected")]);
        }
        for (speech_uid, speakers_list) in speakers {
            speechs
                .get_mut(&speech_uid.to_string())
                .expect("Unexpected uid")
                .update_speakers(&speakers_list);
        }
        let mut speech_list_updated = Vec::new();
        for speech in speechs {
            speech_list_updated.push(speech.1);
        }
        return Ok(speech_list_updated);
    }

    async fn get_all_speech(
        &self,
        page: u16,
        quantity: u16,
    ) -> Result<Vec<Speech>, SpeechRepositoryError> {
        let connection = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;

        let speech_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT uid, name, date, media, status FROM speech LIMIT $1 OFFSET $2;")
                .bind(quantity as i32)
                .bind((page * quantity) as i32)
                .fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;

        let mut speech_list = HashMap::new();
        for speech in speech_result {
            let speech_uid: &str = speech.get("uid");
            let name: &str = speech.get("name");
            let date: DateTime<Utc> = speech.get("date");
            let media: &str = speech.get("media");
            let status: &str = speech.get("status");
            speech_list.insert(
                speech_uid.to_string(),
                Speech::new(
                    &Uuid::from_str(speech_uid)
                        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))?,
                    name,
                    date,
                    &[],
                    &[],
                    media,
                    status
                        .try_into()
                        .map_err(|e| SpeechRepositoryError::InternalError(e))?,
                ),
            );
        }
        let speech_uids = speech_list
            .keys()
            .map(|speech| speech.to_string())
            .collect::<Vec<String>>();

        let speech_person_result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query(
                "SELECT speech_uid, speaker FROM speech_person WHERE speech_uid = ANY($1);",
            )
            .bind(speech_uids)
            .fetch_all(&connection),
        )
        .await
        .map_err(|e| SpeechRepositoryError::InternalError(e.to_string()))??;
        let mut speakers = HashMap::new();
        for speech_person in speech_person_result {
            let uid: &str = speech_person.get("speech_uid");
            let speaker: &str = speech_person.get("speaker");
            speakers
                .entry(uid.to_string())
                .and_modify(|val: &mut Vec<Uuid>| {
                    val.push(Uuid::from_str(speaker).expect("uid format expected"))
                })
                .or_insert(vec![Uuid::from_str(speaker).expect("uid format expected")]);
        }
        for (speech_uid, speakers_list) in speakers {
            speech_list
                .get_mut(&speech_uid.to_string())
                .expect("Unexpected uid")
                .update_speakers(&speakers_list);
        }
        let mut speech_list_updated = Vec::new();
        for speech in speech_list {
            speech_list_updated.push(speech.1);
        }
        return Ok(speech_list_updated);
    }
}

#[cfg(test)]
pub mod tests {
    use std::str::FromStr;

    use chrono::Utc;
    use uuid::Uuid;

    use crate::domain::speech::{
        sentence::Sentence, speech_repository::SpeechRepository, Speech, SpeechStatus,
    };

    use super::PostgresSpeechRepository;

    #[tokio::test]
    async fn test_postgres_speech_in_db() {
        let res = PostgresSpeechRepository::new(
            "postgres://postgres:postgres@localhost/speech_analytics",
            100,
        )
        .await;
        println!("{:?}", res);
        assert_eq!(res.is_ok(), true);
        let repository = res.unwrap();
        let speech_uid = Uuid::from_str("9c01cccd-919b-4c59-84c7-4fef627557b9").unwrap();
        let speaker_1 = Uuid::from_str("d1acaab5-ca6e-4f4f-9019-e065d0638388").unwrap();
        let speaker_2 = Uuid::from_str("349f2610-c5e7-4745-a964-35d3cb8cdc4b").unwrap();
        let sentences = vec![
            Sentence::new(&Uuid::new_v4(), &speaker_1, "Bonjour Michel", false),
            Sentence::new(&Uuid::new_v4(), &speaker_2, "Bonjour Micheline", false),
        ];
        let speech = Speech::new(
            &speech_uid,
            "test_speech",
            Utc::now(),
            &[speaker_1, speaker_2],
            &sentences,
            "TF1",
            SpeechStatus::Pending,
        );
        let res_create_success = repository.create_speech(&speech).await;
        println!("{:?}", res_create_success);
        assert_eq!(res_create_success, Ok(()));
    }
}
