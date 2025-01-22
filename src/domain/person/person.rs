use chrono::NaiveDate;
use uuid::Uuid;

#[derive(Debug)]
pub struct Person {
    uid: Uuid,
    name: String,
    first_name: String,
    birth_date: NaiveDate,
    trust_score: u8,
    lie_quantity: u64,
}

impl Person {
    pub fn new(
        uid: Uuid,
        name: &str,
        first_name: &str,
        birth_date: NaiveDate,
        trust_score: u8,
        lie_quantity: u64,
    ) -> Self {
        Self {
            uid: uid,
            name: name.to_string(),
            first_name: first_name.to_string(),
            birth_date,
            trust_score,
            lie_quantity,
        }
    }

    pub fn uid(&self) -> &Uuid {
        &self.uid
    }
    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn first_name(&self) -> &String {
        &self.first_name
    }
    pub fn birth_date(&self) -> &NaiveDate {
        &self.birth_date
    }
    pub fn trust_score(&self) -> u8 {
        self.trust_score
    }
    pub fn lie_quantity(&self) -> u64 {
        self.lie_quantity
    }
}
