FROM rust:1.84 AS builder
RUN apt-get update && apt-get upgrade -y && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY . .

RUN cargo build --release --bin speech_analytics_api

# Étape 2 : Image finale
FROM debian:bookworm-slim

# Installer les dépendances nécessaires pour exécuter l'application
RUN apt-get update && apt-get upgrade -y && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Définir le dossier de travail
WORKDIR /app

# Copier l'exécutable compilé depuis la première étape
COPY --from=builder /usr/src/app/target/release/speech_analytics_api /app/speech_analytics_api

# Exposer le port (à adapter en fonction de votre webapp)
EXPOSE 3000

# Définir la commande d'exécution
CMD ["./speech_analytics_api"]