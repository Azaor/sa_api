use std::{collections::HashMap, str::FromStr};

use chrono::NaiveDate;
use hyper::Method;
use serde::Deserialize;
use serde_json::{value, Value};
use uuid::Uuid;

use crate::{
    application::api::{
        router::{HttpError, ACCESS_DENIED_ERROR, INTERNAL_ERROR, NOT_FOUND_ERROR},
        token::{AuthToken, Permissions},
    },
    domain::person::{Person, PersonManager, PersonRepositoryError},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePersonInput {
    name: String,
    first_name: String,
    birth_date: String,
}
impl TryFrom<CreatePersonInput> for Person {
    type Error = HttpError<'static>;

    fn try_from(value: CreatePersonInput) -> Result<Self, Self::Error> {
        let birth_date = NaiveDate::from_str(&value.birth_date).map_err(|_| {
            HttpError::new(
                400,
                "InvalidBirthDate",
                "The birth date supplied has an invalid format",
            )
        })?;
        Ok(Person::new(
            Uuid::new_v4(),
            &value.name,
            &value.first_name,
            birth_date,
            0,
            0,
        ))
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GetPersonOutput {
    uid: String,
    name: String,
    first_name: String,
    birth_date: String,
    trust_score: u8,
}

impl From<Person> for GetPersonOutput {
    fn from(value: Person) -> Self {
        return Self {
            uid: value.uid().to_string(),
            name: value.name().clone(),
            first_name: value.first_name().clone(),
            birth_date: value.birth_date().to_string(),
            trust_score: value.trust_score(),
        };
    }
}

impl From<PersonRepositoryError> for HttpError<'static> {
    fn from(value: PersonRepositoryError) -> Self {
        match value {
            PersonRepositoryError::PersonNotFound => {
                HttpError::new(404, "PersonNotFound", "The person requested is not found")
            }
            PersonRepositoryError::PersonAlreadyExists => HttpError::new(
                409,
                "PersonAlreadyExists",
                "The person you try to create already exists.",
            ),
            PersonRepositoryError::InternalError(e) => {
                println!(
                    "An internal error occured while making an action on Persons: {}",
                    e
                );
                INTERNAL_ERROR
            }
        }
    }
}

pub async fn router(
    path: &str,
    query_params: &HashMap<String, String>,
    method: &Method,
    token: &AuthToken,
    body: Value,
    person_manager: &PersonManager,
) -> Result<Value, HttpError<'static>> {
    match (method, path) {
        (&Method::POST, "") => {
            if !token.permissions().contains(&Permissions::CreatePerson) {
                return Err(ACCESS_DENIED_ERROR);
            }
            let create_person_input: CreatePersonInput =
                serde_json::from_value(body).map_err(|_| {
                    HttpError::new(
                        400,
                        "InvalidFormat",
                        "The body format is invalid. Please refer to the documentation",
                    )
                })?;
            person_manager
                .create_person(create_person_input.try_into()?)
                .await?;
            Ok(Value::Null)
        }
        (&Method::GET, "") => {
            if !token.permissions().contains(&Permissions::GetPerson) {
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
            let people = person_manager.get_people(page, quantity).await?;
            let people_json: Vec<GetPersonOutput> = people
                .into_iter()
                .map(|p| GetPersonOutput::from(p))
                .collect();
            return Ok(value::to_value(people_json).map_err(|e| {
                println!(
                    "An internal error occured while converting persons to value: {:?}",
                    e
                );
                INTERNAL_ERROR
            })?);
        }
        (&Method::GET, _) => {
            if !token.permissions().contains(&Permissions::GetPerson) {
                return Err(ACCESS_DENIED_ERROR);
            }
            // Get a specific person
            let uid_proposed = Uuid::from_str(path).map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidUID",
                    "The UID you provided seems not to ba a valid UUIDv4",
                )
            })?;
            let person_found: GetPersonOutput =
                person_manager.get_person_by_id(&uid_proposed).await?.into();
            let response_body = value::to_value(person_found).map_err(|e| {
                println!(
                    "An internal error occured while converting person to value: {:?}",
                    e
                );
                INTERNAL_ERROR
            })?;
            Ok(response_body)
        }
        (&Method::DELETE, _) => {
            if !token.permissions().contains(&Permissions::DeletePerson) {
                return Err(ACCESS_DENIED_ERROR);
            }
            // Delete a specific person
            let uid_proposed = Uuid::from_str(path).map_err(|_| {
                HttpError::new(
                    400,
                    "InvalidUID",
                    "The UID you provided seems not to ba a valid UUIDv4",
                )
            })?;
            person_manager.delete_person(&uid_proposed).await?;
            Ok(Value::Null)
        }
        (_, _) => return Err(NOT_FOUND_ERROR),
    }
}
