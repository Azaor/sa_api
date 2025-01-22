use std::str::FromStr;

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub enum Permissions {
    GetSpeech,
    CreateSpeech,
    DeleteSpeech,
    UpdateSpeech,
    GetPerson,
    CreatePerson,
    UpdatePerson,
    DeletePerson,
}

impl FromStr for Permissions {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GetSpeech" => Ok(Permissions::GetSpeech),
            "CreateSpeech" => Ok(Permissions::CreateSpeech),
            "DeleteSpeech" => Ok(Permissions::DeleteSpeech),
            "UpdateSpeech" => Ok(Permissions::UpdateSpeech),
            "GetPerson" => Ok(Permissions::GetPerson),
            "CreatePerson" => Ok(Permissions::CreatePerson),
            "UpdatePerson" => Ok(Permissions::UpdatePerson),
            "DeletePerson" => Ok(Permissions::DeletePerson),
            _ => Err(format!("Invalid permission: {}", s)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuthToken {
    user_id: Option<String>,
    username: Option<String>,
    permissions: Vec<Permissions>,
}

impl Default for AuthToken {
    fn default() -> Self {
        Self {
            user_id: Default::default(),
            username: Default::default(),
            permissions: vec![Permissions::GetPerson, Permissions::GetSpeech],
        }
    }
}

impl AuthToken {
    pub fn new(
        user_id: Option<String>,
        username: Option<String>,
        permissions: Vec<Permissions>,
    ) -> Self {
        return Self {
            user_id,
            username,
            permissions,
        };
    }

    pub fn user_id(&self) -> String {
        return self.user_id.clone().unwrap_or("anonymous".to_owned());
    }
    pub fn username(&self) -> String {
        return self.username.clone().unwrap_or("Unknown_user".to_owned());
    }
    pub fn permissions(&self) -> &Vec<Permissions> {
        return &self.permissions;
    }
}
