use std::{str::FromStr, time::Duration};

use chrono::NaiveDate;
use sqlx::{postgres::PgRow, Error, PgPool, Row};
use tokio::{runtime::Runtime, time};
use uuid::Uuid;

use crate::domain::person::{Person, PersonRepository, PersonRepositoryError};

impl From<Error> for PersonRepositoryError {
    fn from(value: Error) -> Self {
        match value {
            Error::Database(database_error) => {
                if database_error.is_unique_violation() || database_error.is_check_violation() {
                    return Self::PersonAlreadyExists;
                }
                return Self::InternalError(database_error.to_string());
            }
            Error::RowNotFound => {
                return Self::PersonNotFound;
            }
            _ => return Self::InternalError(value.to_string()),
        }
    }
}

impl TryFrom<PgRow> for Person {
    type Error = PersonRepositoryError;

    fn try_from(value: PgRow) -> Result<Self, Self::Error> {
        let uid: &str = value.try_get("uid")?;
        let name: &str = value.try_get("name")?;
        let first_name: &str = value.try_get("first_name")?;
        let birth_date: NaiveDate = value.try_get("birth_date")?;
        let trust_score: i16 = value.try_get("trust_score")?;
        let lie_quantity: i64 = value.try_get("lie_quantity")?;
        return Ok(Person::new(
            Uuid::from_str(uid).map_err(|_| {
                PersonRepositoryError::InternalError(format!("Invalid uid format for user {}", uid))
            })?,
            name.trim(),
            first_name.trim(),
            birth_date,
            trust_score as u8,
            lie_quantity as u64,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct PostgresPersonRepository {
    url: String,
    timeout: u64,
}

async fn init_table_async(url: &str, timeout: u64) -> Result<(), PersonRepositoryError> {
    let connection = time::timeout(Duration::from_millis(timeout), PgPool::connect(url))
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
    let create_table_query = r#"CREATE TABLE IF NOT EXISTS person (
        uid CHAR(36) PRIMARY KEY,
        name CHAR(50),
        first_name CHAR(50),
        birth_date DATE,
        trust_score SMALLINT,
        lie_quantity BIGINT,
        CONSTRAINT unique_identity UNIQUE (name, first_name, birth_date)
    )"#;
    let _result = time::timeout(
        Duration::from_millis(timeout),
        sqlx::query(create_table_query).execute(&connection),
    )
    .await
    .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
    Ok(())
}

impl PostgresPersonRepository {
    pub async fn new(url: &str, timeout: u64) -> Result<Self, PersonRepositoryError> {
        init_table_async(url, timeout).await?;
        Ok(Self {
            url: url.to_string(),
            timeout,
        })
    }
}

#[async_trait::async_trait]
impl PersonRepository for PostgresPersonRepository {
    async fn create_person(&self, person: &Person) -> Result<(), PersonRepositoryError> {
        let connection = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        let _result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("INSERT INTO person VALUES ($1, $2, $3, $4, $5, $6);")
                .bind(person.uid().to_string())
                .bind(person.name())
                .bind(person.first_name())
                .bind(person.birth_date().to_string())
                .bind(person.trust_score() as i32)
                .bind(person.lie_quantity() as i32)
                .execute(&connection),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        Ok(())
    }

    async fn update_person(&self, _person: &Person) -> Result<(), PersonRepositoryError> {
        todo!()
    }

    async fn get_person_by_id(&self, uid: &Uuid) -> Result<Person, PersonRepositoryError> {
        let connection: sqlx::Pool<sqlx::Postgres> = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        let person_found = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT uid, name, first_name, birth_date, trust_score, lie_quantity FROM person WHERE uid = $1;").bind(uid.to_string()).fetch_one(&connection),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        return Ok(person_found.try_into()?);
    }

    async fn get_people(
        &self,
        page: u16,
        quantity: u16,
    ) -> Result<Vec<Person>, PersonRepositoryError> {
        let connection: sqlx::Pool<sqlx::Postgres> = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        let result = time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("SELECT uid, name, first_name, birth_date, trust_score, lie_quantity FROM person LIMIT $1 OFFSET $2;").bind(quantity as i32).bind((page*quantity) as i32).fetch_all(&connection),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        return Ok(result.into_iter().fold(Vec::new(), |mut acc, v| {
            let convert = v.try_into();
            if convert.is_ok() {
                acc.push(convert.unwrap());
            }
            acc
        }));
    }

    async fn delete_person(&self, uid: &Uuid) -> Result<(), PersonRepositoryError> {
        let connection: sqlx::Pool<sqlx::Postgres> = time::timeout(
            Duration::from_millis(self.timeout),
            PgPool::connect(&self.url),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        time::timeout(
            Duration::from_millis(self.timeout),
            sqlx::query("DELETE FROM person WHERE uid = $1")
                .bind(uid.to_string())
                .execute(&connection),
        )
        .await
        .map_err(|e| PersonRepositoryError::InternalError(e.to_string()))??;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use std::str::FromStr;

    use crate::domain::person::{Person, PersonRepository, PersonRepositoryError};
    use chrono::NaiveDate;
    use uuid::Uuid;

    use super::PostgresPersonRepository;

    #[tokio::test]
    async fn test_postgres_person_in_db() {
        let res = PostgresPersonRepository::new(
            "postgres://postgres:postgres@localhost/speech_analytics",
            100,
        )
        .await;
        assert_eq!(res.is_ok(), true);
        let repository = res.unwrap();
        let person_uid = Uuid::from_str("9c01cccd-919b-4c59-84c7-4fef627557b9").unwrap();
        let person = Person::new(
            person_uid,
            "test_name",
            "test_first_name",
            NaiveDate::from_isoywd_opt(2000, 1, chrono::Weekday::Mon).unwrap(),
            0,
            0,
        );
        let res_create_success = repository.create_person(&person).await;
        assert_eq!(res_create_success, Ok(()));
        let res_create_err_duplicate = repository.create_person(&person).await;
        assert_eq!(
            res_create_err_duplicate,
            Err(PersonRepositoryError::PersonAlreadyExists)
        );
        let res_get_person = repository.get_person_by_id(&person_uid).await;
        assert_eq!(res_get_person.is_ok(), true);
        let person_fetched = res_get_person.unwrap();
        assert_eq!(person_fetched.name(), person.name());
        assert_eq!(person_fetched.first_name(), person.first_name());
        assert_eq!(person_fetched.birth_date(), person.birth_date());
        assert_eq!(person_fetched.lie_quantity(), person.lie_quantity());
        assert_eq!(person_fetched.trust_score(), person.trust_score());
        let res_delete_person = repository.delete_person(&person_uid).await;
        assert_eq!(res_delete_person.is_ok(), true);
        let res_get_person_not_found = repository.get_person_by_id(&person_uid).await;
        assert_eq!(res_get_person_not_found.is_err(), true);
        let err = res_get_person_not_found.unwrap_err();
        assert_eq!(err, PersonRepositoryError::PersonNotFound);
    }
}
