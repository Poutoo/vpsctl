//! Conteneurs Docker : liste (`docker ps -a --format '{{json .}}'`, parsing
//! dans des fonctions pures), start, stop, restart, rm et flux `docker logs -f`.
//! Les conteneurs sont manipulés par ID (hexadécimal validé), jamais par nom.
