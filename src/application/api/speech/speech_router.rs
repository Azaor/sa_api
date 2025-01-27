use std::{collections::HashMap, str::FromStr};

use chrono::DateTime;
use hyper::Method;
use serde::{Deserialize, Serialize};
use serde_json::{value, Value};
use uuid::Uuid;

use crate::{
    application::api::{
        router::{HttpError, ACCESS_DENIED_ERROR, INTERNAL_ERROR, NOT_FOUND_ERROR},
        token::{AuthToken, Permissions},
    },
    domain::speech::{
        manager::SpeechManager, sentence::Sentence, speech_repository::SpeechRepositoryError,
        Speech, SpeechStatus,
    },
};

impl From<SpeechRepositoryError> for HttpError<'static> {
    fn from(value: SpeechRepositoryError) -> Self {
        match value {
            SpeechRepositoryError::PersonError(person_repository_error) => {
                person_repository_error.into()
            }
            SpeechRepositoryError::SpeechNotFound => {
                HttpError::new(404, "SpeechNotFound", "The speech requested is not found")
            }
            SpeechRepositoryError::SpeechAlreadyExists => HttpError::new(
                409,
                "SpeechAlreadyExists",
                "The speech you try to create already exists.",
            ),
            SpeechRepositoryError::InternalError(e) => {
                println!("Internal Error: {}", e);
                INTERNAL_ERROR
            }
        }
    }
}

#[derive(Deserialize)]
pub struct CreateSpeechSentenceInput {
    speaker: String,
    text: String,
    interrupted: bool,
}

impl TryFrom<CreateSpeechSentenceInput> for Sentence {
    type Error = HttpError<'static>;

    fn try_from(value: CreateSpeechSentenceInput) -> Result<Self, Self::Error> {
        let speaker_id = Uuid::from_str(&value.speaker).map_err(|_| {
            HttpError::new(400, "InvalidUID", "A speaker uid have an invalid format")
        })?;
        return Ok(Self::new(
            &Uuid::new_v4(),
            &speaker_id,
            &value.text,
            value.interrupted,
        ));
    }
}

#[derive(Deserialize)]
pub struct CreateSpeechInput {
    name: String,
    date: String,
    speakers: Vec<String>,
    sentences: Vec<CreateSpeechSentenceInput>,
    media: String,
}

impl TryFrom<CreateSpeechInput> for Speech {
    type Error = HttpError<'static>;

    fn try_from(value: CreateSpeechInput) -> Result<Self, Self::Error> {
        let mut sentences = Vec::new();
        for s in value.sentences {
            sentences.push(s.try_into()?);
        }
        let date = DateTime::from_str(&value.date).map_err(|_| {
            HttpError::new(
                400,
                "InvalidDate",
                "The date provided is invalid. Please be sure to provide an ISO 8601 date.",
            )
        })?;
        let mut speakers = Vec::new();
        for speaker in value.speakers {
            speakers.push(Uuid::from_str(&speaker).map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidSpeakersUid",
                    "One of the speaker uid provided have an invalid format",
                )
            })?);
        }
        return Ok(Self::new(
            &Uuid::new_v4(),
            &value.name,
            date,
            &speakers,
            &sentences,
            &value.media,
            SpeechStatus::Pending,
        ));
    }
}

#[derive(Serialize)]
struct GetSpeechSentence {
    uid: String,
    speaker: String,
    text: String,
    interrupted: bool,
}

impl From<Sentence> for GetSpeechSentence {
    fn from(value: Sentence) -> Self {
        return GetSpeechSentence {
            uid: value.uid().to_string(),
            speaker: value.speaker().to_string(),
            text: value.text().clone(),
            interrupted: value.interrupted(),
        };
    }
}

#[derive(Serialize)]
struct GetSpeechById {
    uid: String,
    name: String,
    date: String,
    media: String,
    speakers: Vec<String>,
    sentences: Vec<GetSpeechSentence>,
}

impl From<Speech> for GetSpeechById {
    fn from(value: Speech) -> Self {
        Self {
            uid: value.uid().to_string(),
            name: value.name().clone(),
            date: value.date().to_rfc3339(),
            media: value.media().clone(),
            speakers: value.speakers().iter().map(|v| v.to_string()).collect(),
            sentences: value
                .sentences()
                .iter()
                .map(|e| GetSpeechSentence::from(e.clone()))
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct GetSpeech {
    uid: String,
    name: String,
    date: String,
    speakers: Vec<String>,
    media: String,
}

impl From<Speech> for GetSpeech {
    fn from(value: Speech) -> Self {
        Self {
            uid: value.uid().to_string(),
            name: value.name().clone(),
            date: value.date().to_rfc3339(),
            media: value.media().clone(),
            speakers: value.speakers().iter().map(|v| v.to_string()).collect(),
        }
    }
}

pub async fn router(
    path: &str,
    query_params: &HashMap<String, String>,
    method: &Method,
    token: &AuthToken,
    body: Value,
    speech_manager: &SpeechManager,
) -> Result<Value, HttpError<'static>> {
    match (method, path) {
        (&Method::POST, "") => {
            if !token.permissions().contains(&Permissions::CreateSpeech) {
                return Err(ACCESS_DENIED_ERROR);
            }
            let create_speech_input: CreateSpeechInput =
                serde_json::from_value(body).map_err(|_| {
                    HttpError::new(
                        400,
                        "InvalidFormat",
                        "The body format is invalid. Please refer to the documentation",
                    )
                })?;
            speech_manager
                .create_speech(create_speech_input.try_into()?)
                .await?;
            Ok(Value::Null)
        }
        (&Method::GET, "") => {
            if !token.permissions().contains(&Permissions::GetSpeech) {
                return Err(ACCESS_DENIED_ERROR);
            }
            // Get all Peoples
            let page_raw = match query_params.get("page") {
                Some(v) => v,
                None => &"0".to_owned(),
            };
            let quantity_raw = match query_params.get("quantity") {
                Some(v) => v,
                None => &"10".to_owned(),
            };
            let speakers_raw = extract_array_in_query("speakers", query_params)?;
            let page = page_raw.parse::<u16>().map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidPageParam",
                    "The page parameter provided must be an integer > 0",
                )
            })?;
            let quantity = quantity_raw.parse::<u16>().map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidQuantityParam",
                    "The quantity parameter provided must be an integer > 0",
                )
            })?;

            let mut speakers_uid = Vec::new();
            for speaker_uid in speakers_raw {
                speakers_uid.push(Uuid::from_str(&speaker_uid).map_err(|_| {
                    HttpError::new(
                        400,
                        "InvalidUid",
                        "The uid provided seems invalid, please check it again",
                    )
                })?);
            }
            let speech: Vec<GetSpeech> = speech_manager
                .get_speech(page, quantity, &speakers_uid)
                .await?
                .into_iter()
                .map(|s| s.into())
                .collect();

            Ok(value::to_value(speech).map_err(|e| {
                println!(
                    "An internal error occured while converting speeches to value: {}",
                    e
                );
                INTERNAL_ERROR
            })?)
        }
        (&Method::GET, _) => {
            if !token.permissions().contains(&Permissions::GetSpeech) {
                return Err(ACCESS_DENIED_ERROR);
            }
            let uid = Uuid::from_str(path).map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidUid",
                    "The uid provided seems invalid, please check it again",
                )
            })?;
            let speech_found: GetSpeechById = speech_manager.get_speech_by_id(uid).await?.into();
            Ok(value::to_value(speech_found).map_err(|e| {
                println!(
                    "An internal error occured while converting speech by id: {:?}",
                    e
                );
                INTERNAL_ERROR
            })?)
        }
        (&Method::DELETE, _) => {
            if !token.permissions().contains(&Permissions::DeleteSpeech) {
                return Err(ACCESS_DENIED_ERROR);
            }
            let uid = Uuid::from_str(path).map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidUid",
                    "The uid provided seems invalid, please check it again",
                )
            })?;
            speech_manager.delete_speech(uid).await?;
            Ok(Value::Null)
        }
        (_, _) => return Err(NOT_FOUND_ERROR),
    }
}

fn extract_array_in_query(
    array_field: &str,
    query_params: &HashMap<String, String>,
) -> Result<Vec<String>, HttpError<'static>> {
    let array_raw = match query_params.get(array_field) {
        Some(v) => v,
        None => {
            return Ok(Vec::new());
        }
    };
    let array_decomposed = match array_raw.split("%5B").skip(1).next() {
        Some(v) => v,
        None => {
            return Err(HttpError::new(
                400,
                "InvalidArrayParam",
                "The array query parameter given is an invalid format.",
            ))
        }
    };
    let array_decomposed = match array_decomposed.split("%5D").next() {
        Some(v) => v,
        None => {
            return Err(HttpError::new(
                400,
                "InvalidArrayParam",
                "The array query parameter given is an invalid format.",
            ))
        }
    };
    return Ok(array_decomposed
        .split(",")
        .map(|v| v.to_string())
        .collect::<Vec<String>>());
}
