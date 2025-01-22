use jsonwebtoken::DecodingKey;
use lazy_static::lazy_static;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// Structure des certificats Keycloak
#[derive(Deserialize)]
struct KeycloakCerts {
    keys: Vec<KeycloakKey>,
}

// Structure d'une clé Keycloak
#[derive(Deserialize)]
struct KeycloakKey {
    kid: String, // Key ID
    n: String,   // Modulus
    e: String,   // Exponent
    kty: String, // Key type (e.g., RSA)
}

// Structure pour gérer le cache des clés
struct CachedKeys {
    keys: HashMap<String, DecodingKey>, // Les clés sont stockées ici
    last_fetched: Instant,              // Dernière récupération des clés
}

// Initialisation d'un cache global
lazy_static! {
    static ref KEYCLOAK_KEYS_CACHE: Mutex<CachedKeys> = Mutex::new(CachedKeys {
        keys: HashMap::new(),
        last_fetched: Instant::now() - Duration::from_secs(3600), // Initialisé à il y a 1h
    });
}

/// Fonction pour récupérer les clés Keycloak avec mise en cache
pub async fn get_keycloak_keys() -> Result<HashMap<String, DecodingKey>, Box<dyn std::error::Error>>
{
    let mut cache = KEYCLOAK_KEYS_CACHE.lock().await;

    // Vérifiez si le cache est expiré (par exemple, 1 heure)
    if cache.last_fetched.elapsed() < Duration::from_secs(3600) {
        return Ok(cache.keys.clone());
    }

    // Construire l'URL JWKS (JSON Web Key Set) de Keycloak
    let jwks_url = format!("{}", std::env::var("KEYCLOAK_CERTS_URL")?);

    // Effectuer une requête HTTP pour récupérer les clés
    let client = Client::new();
    let response = client.get(&jwks_url).send().await?;
    let keycloak_certs: KeycloakCerts = response.json().await?;

    // Transformer les clés en un format utilisable par la bibliothèque jsonwebtoken
    let mut keys = HashMap::new();
    for key in keycloak_certs.keys {
        if key.kty == "RSA" {
            let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)?;
            keys.insert(key.kid, decoding_key);
        }
    }

    // Mettre à jour le cache
    cache.keys = keys.clone();
    cache.last_fetched = Instant::now();

    Ok(keys)
}
