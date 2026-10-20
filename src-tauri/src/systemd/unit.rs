//! Fichier d'unité d'un service créé depuis l'application : validation de
//! `ServiceDef` et production du texte (fonctions pures). L'aperçu du
//! formulaire et le fichier écrit sur le serveur passent par `render_unit` :
//! ils sont toujours identiques.
//!
//! Règles de lecture de systemd prises en compte (vérifiées dans le source de
//! systemd 259) et règles de chaque champ : `docs/03-architecture.md`,
//! « Fichiers d'unité systemd ». En résumé :
//!
//! - aucun caractère de contrôle (retour à la ligne, `\r`, caractère nul…),
//!   aucune valeur terminée par `\` (continuation de ligne) : un champ ne
//!   peut pas ajouter de directive ;
//! - `%` (spécificateurs) devient `%%` partout ; le seul spécificateur
//!   produit est le `%h` de la conversion de `~/` dans le dossier de travail ;
//! - `ExecStart=` garde la syntaxe de systemd (guillemets, échappements) mais
//!   `$` devient `$$` : aucune substitution de variable ; les préfixes
//!   spéciaux (`-`, `@`, `:`, `+`, `!`, `|`) et le `;` isolé sont refusés.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;

/// Longueur maximale du nom saisi (sans `.service`).
pub const MAX_NAME_CHARS: usize = 64;
pub const MAX_DESCRIPTION_CHARS: usize = 256;
/// Longueur maximale de la commande, du dossier et d'une valeur, en octets.
pub const MAX_VALUE_BYTES: usize = 4096;
pub const MAX_ENV_VARS: usize = 32;
pub const MAX_ENV_KEY_CHARS: usize = 128;

/// Service à créer, saisi dans le formulaire.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceDef {
    /// Sans le suffixe `.service` : `^[a-z0-9][a-z0-9-]*$`.
    pub name: String,
    /// Vide : ligne `Description=` omise.
    pub description: String,
    pub exec_start: String,
    /// `~`, `~/chemin` ou chemin absolu.
    pub working_dir: String,
    pub restart: RestartPolicy,
    /// Dans l'ordre saisi ; clés sans doublon.
    pub env: Vec<EnvVar>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RestartPolicy {
    No,
    OnFailure,
    Always,
}

impl RestartPolicy {
    fn value(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::OnFailure => "on-failure",
            Self::Always => "always",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

/// Erreur d'un champ : le message commence par le nom du champ.
fn field_error(field: &str, message: impl std::fmt::Display) -> AppError {
    AppError::invalid_input(format!("{field} : {message}"))
}

/// Nom d'unité d'un nom saisi : `<nom>.service`, après validation par
/// `^[a-z0-9][a-z0-9-]*$` (64 caractères au plus).
pub fn unit_name(name: &str) -> Result<String, AppError> {
    const FIELD: &str = "Nom";
    if name.ends_with(".service") {
        return Err(field_error(FIELD, "saisissez-le sans « .service »"));
    }
    let mut bytes = name.bytes();
    let valid_start = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let valid_rest = bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid_start || !valid_rest {
        return Err(field_error(
            FIELD,
            "minuscules, chiffres et tirets, sans tiret au début",
        ));
    }
    if name.len() > MAX_NAME_CHARS {
        return Err(field_error(
            FIELD,
            format!("{MAX_NAME_CHARS} caractères au plus"),
        ));
    }
    Ok(format!("{name}.service"))
}

/// Texte complet du fichier d'unité, après validation de chaque champ.
pub fn render_unit(def: &ServiceDef) -> Result<String, AppError> {
    unit_name(&def.name)?;
    let description = description_value(&def.description)?;
    let exec_start = exec_start_value(&def.exec_start)?;
    let working_dir = working_dir_value(&def.working_dir)?;
    let env = env_lines(&def.env)?;

    let mut text = String::from("[Unit]\n");
    if let Some(description) = description {
        text.push_str(&format!("Description={description}\n"));
    }
    text.push_str("\n[Service]\nType=simple\n");
    text.push_str(&format!("WorkingDirectory={working_dir}\n"));
    text.push_str(&format!("ExecStart={exec_start}\n"));
    text.push_str(&format!("Restart={}\n", def.restart.value()));
    text.push_str("RestartSec=5\n");
    for line in env {
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str("\n[Install]\nWantedBy=default.target\n");
    Ok(text)
}

/// `%` → `%%` : aucun spécificateur systemd ne vient de la saisie.
pub fn escape_specifiers(value: &str) -> String {
    value.replace('%', "%%")
}

/// Refuse tout caractère de contrôle : retour à la ligne, `\r` et caractère
/// nul terminent une ligne pour systemd, les autres n'ont rien à faire dans
/// un champ d'une ligne.
fn check_single_line(field: &str, value: &str) -> Result<(), AppError> {
    if value.chars().any(char::is_control) {
        return Err(field_error(
            field,
            "retour à la ligne et caractères de contrôle interdits",
        ));
    }
    Ok(())
}

/// Une ligne terminée par `\` est jointe à la suivante par systemd.
fn check_no_trailing_backslash(field: &str, value: &str) -> Result<(), AppError> {
    if value.ends_with('\\') {
        return Err(field_error(field, "ne doit pas se terminer par « \\ »"));
    }
    Ok(())
}

fn check_max_bytes(field: &str, value: &str) -> Result<(), AppError> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(field_error(
            field,
            format!("{MAX_VALUE_BYTES} octets au plus"),
        ));
    }
    Ok(())
}

/// Description : facultative (`None` si vide), espaces de bord retirés
/// (systemd les retirerait aussi), `%` échappé.
fn description_value(description: &str) -> Result<Option<String>, AppError> {
    const FIELD: &str = "Description";
    let description = description.trim();
    if description.is_empty() {
        return Ok(None);
    }
    check_single_line(FIELD, description)?;
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(field_error(
            FIELD,
            format!("{MAX_DESCRIPTION_CHARS} caractères au plus"),
        ));
    }
    check_no_trailing_backslash(FIELD, description)?;
    Ok(Some(escape_specifiers(description)))
}

/// Dossier de travail : `~` tel quel, `~/chemin` → `%h/chemin`, ou chemin
/// absolu. `WorkingDirectory=` ne traite ni guillemets ni échappements, mais
/// exige un chemin normalisé : `\`, `.` et `..` sont refusés.
fn working_dir_value(dir: &str) -> Result<String, AppError> {
    const FIELD: &str = "Dossier de travail";
    let dir = dir.trim();
    if dir.is_empty() {
        return Err(field_error(FIELD, "requis (~ pour le dossier personnel)"));
    }
    check_single_line(FIELD, dir)?;
    check_max_bytes(FIELD, dir)?;
    if dir.contains('\\') {
        return Err(field_error(FIELD, "« \\ » interdit"));
    }
    if dir == "~" {
        return Ok("~".into());
    }
    let (prefix, rest) = if let Some(rest) = dir.strip_prefix("~/") {
        ("%h/", rest)
    } else if let Some(rest) = dir.strip_prefix('/') {
        ("/", rest)
    } else {
        return Err(field_error(
            FIELD,
            "chemin absolu, ~ ou chemin commençant par ~/",
        ));
    };
    if rest.split('/').any(|c| c == "." || c == "..") {
        return Err(field_error(FIELD, "composants « . » et « .. » interdits"));
    }
    if prefix == "%h/" && rest.split('/').all(str::is_empty) {
        return Ok("~".into());
    }
    Ok(format!("{prefix}{}", escape_specifiers(rest)))
}

/// Un mot de la commande, découpé comme le fait systemd (`extract_first_word`
/// avec retrait des guillemets et des échappements).
#[derive(Debug, PartialEq, Eq)]
struct Word<'a> {
    /// Texte tel que saisi.
    raw: &'a str,
    /// Texte après retrait des guillemets ; `\x` donne `x` (approximation
    /// suffisante : un `\` est refusé dans le programme).
    text: String,
}

/// Découpe la commande en mots, comme systemd : espaces hors guillemets,
/// `'…'` et `"…"` n'importe où dans un mot, `\` qui protège le caractère
/// suivant (y compris entre guillemets).
fn split_words(command: &str) -> Result<Vec<Word<'_>>, &'static str> {
    let mut words = Vec::new();
    let mut chars = command.char_indices().peekable();
    loop {
        while chars.next_if(|(_, c)| *c == ' ').is_some() {}
        let Some(&(start, _)) = chars.peek() else {
            return Ok(words);
        };
        let mut text = String::new();
        let mut quote: Option<char> = None;
        let mut end = command.len();
        while let Some((i, c)) = chars.next() {
            match (quote, c) {
                (_, '\\') => match chars.next() {
                    Some((_, escaped)) => text.push(escaped),
                    None => return Err("ne doit pas se terminer par « \\ »"),
                },
                (Some(q), c) if c == q => quote = None,
                (Some(_), c) => text.push(c),
                (None, '\'' | '"') => quote = Some(c),
                (None, ' ') => {
                    end = i;
                    break;
                }
                (None, c) => text.push(c),
            }
        }
        if quote.is_some() {
            return Err("guillemet non fermé");
        }
        words.push(Word {
            raw: &command[start..end],
            text,
        });
    }
}

/// Commande : syntaxe de systemd conservée (guillemets, échappements), sans
/// substitution (`%` → `%%`, `$` → `$$`), sans préfixe spécial ni `;`
/// isolé. Le programme est un chemin absolu ou un simple nom.
fn exec_start_value(command: &str) -> Result<String, AppError> {
    const FIELD: &str = "Commande";
    let command = command.trim();
    if command.is_empty() {
        return Err(field_error(FIELD, "requise"));
    }
    check_single_line(FIELD, command)?;
    check_max_bytes(FIELD, command)?;
    check_no_trailing_backslash(FIELD, command)?;
    let words = split_words(command).map_err(|message| field_error(FIELD, message))?;

    let program = &words[0];
    if program.raw.contains('\\') {
        return Err(field_error(FIELD, "« \\ » interdit dans le programme"));
    }
    match program.text.chars().next() {
        None => return Err(field_error(FIELD, "programme vide")),
        Some('~') => {
            return Err(field_error(
                FIELD,
                "systemd ne développe pas ~ : chemin absolu du programme requis",
            ))
        }
        Some(prefix @ ('-' | '@' | ':' | '+' | '!' | '|' | ';')) => {
            return Err(field_error(
                FIELD,
                format!(
                    "le préfixe spécial « {prefix} » de systemd n'est pas pris en charge : \
                     commencez par le programme"
                ),
            ))
        }
        Some(_) => {}
    }
    if program.text.contains('/') && !program.text.starts_with('/') {
        return Err(field_error(
            FIELD,
            "le programme est un chemin absolu ou un simple nom",
        ));
    }
    if words[1..].iter().any(|w| w.raw == ";") {
        return Err(field_error(
            FIELD,
            "un « ; » isolé sépare deux commandes pour systemd : écrivez \\; \
             ou passez par /bin/sh -c '…'",
        ));
    }
    Ok(escape_specifiers(command).replace('$', "$$"))
}

/// Clé de variable d'environnement : `^[A-Za-z_][A-Za-z0-9_]*$`.
pub fn is_valid_env_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// `Environment="CLE=valeur"`, avec `\` → `\\`, `"` → `\"` et `%` → `%%`.
fn env_lines(env: &[EnvVar]) -> Result<Vec<String>, AppError> {
    const FIELD: &str = "Variables d'environnement";
    if env.len() > MAX_ENV_VARS {
        return Err(field_error(FIELD, format!("{MAX_ENV_VARS} au plus")));
    }
    let mut seen = std::collections::HashSet::new();
    env.iter()
        .map(|var| {
            if !is_valid_env_key(&var.key) || var.key.len() > MAX_ENV_KEY_CHARS {
                return Err(field_error(
                    FIELD,
                    format!(
                        "clé {:?} invalide : lettres, chiffres et _, sans chiffre au début, \
                         {MAX_ENV_KEY_CHARS} caractères au plus",
                        var.key
                    ),
                ));
            }
            if !seen.insert(var.key.as_str()) {
                return Err(field_error(FIELD, format!("clé {} en double", var.key)));
            }
            let field = format!("Variable {}", var.key);
            check_single_line(&field, &var.value)?;
            check_max_bytes(&field, &var.value)?;
            let value = escape_specifiers(&var.value.replace('\\', r"\\").replace('"', r#"\""#));
            Ok(format!("Environment=\"{}={value}\"", var.key))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn def() -> ServiceDef {
        ServiceDef {
            name: "bot-discord".into(),
            description: "Bot Discord".into(),
            exec_start: "/usr/bin/node dist/server.js".into(),
            working_dir: "/srv/bot-discord".into(),
            restart: RestartPolicy::OnFailure,
            env: vec![EnvVar {
                key: "NODE_ENV".into(),
                value: "production".into(),
            }],
        }
    }

    fn var(key: &str, value: &str) -> EnvVar {
        EnvVar {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Message de l'erreur `InvalidInput`.
    fn refused(def: &ServiceDef) -> String {
        match render_unit(def) {
            Err(AppError::InvalidInput { message }) => message,
            other => panic!("attendu InvalidInput, reçu {other:?}"),
        }
    }

    fn with_exec(command: &str) -> ServiceDef {
        ServiceDef {
            exec_start: command.into(),
            ..def()
        }
    }

    fn exec_line(command: &str) -> String {
        render_unit(&with_exec(command))
            .unwrap()
            .lines()
            .find_map(|l| l.strip_prefix("ExecStart=").map(str::to_owned))
            .unwrap()
    }

    fn working_dir_line(dir: &str) -> String {
        let def = ServiceDef {
            working_dir: dir.into(),
            ..def()
        };
        render_unit(&def)
            .unwrap()
            .lines()
            .find_map(|l| l.strip_prefix("WorkingDirectory=").map(str::to_owned))
            .unwrap()
    }

    // --- Textes de référence complets -----------------------------------

    #[test]
    fn reference_unit_matches_the_template() {
        assert_eq!(
            render_unit(&def()).unwrap(),
            "[Unit]\n\
             Description=Bot Discord\n\
             \n\
             [Service]\n\
             Type=simple\n\
             WorkingDirectory=/srv/bot-discord\n\
             ExecStart=/usr/bin/node dist/server.js\n\
             Restart=on-failure\n\
             RestartSec=5\n\
             Environment=\"NODE_ENV=production\"\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n"
        );
    }

    /// Exemple de `docs/03-architecture.md`.
    #[test]
    fn reference_unit_with_escapes() {
        let def = ServiceDef {
            name: "backup".into(),
            description: "Sauvegarde 100% locale".into(),
            exec_start: r#"/bin/sh -c 'tar czf "a-$(date +%Y).tgz" data'"#.into(),
            working_dir: "~/backup".into(),
            restart: RestartPolicy::OnFailure,
            env: vec![var("TOKEN", r#"a"b\c"#)],
        };
        assert_eq!(
            render_unit(&def).unwrap(),
            r#"[Unit]
Description=Sauvegarde 100%% locale

[Service]
Type=simple
WorkingDirectory=%h/backup
ExecStart=/bin/sh -c 'tar czf "a-$$(date +%%Y).tgz" data'
Restart=on-failure
RestartSec=5
Environment="TOKEN=a\"b\\c"

[Install]
WantedBy=default.target
"#
        );
    }

    #[test]
    fn minimal_unit_omits_description_and_environment() {
        let def = ServiceDef {
            name: "a".into(),
            description: "   ".into(),
            exec_start: "  sleep infinity  ".into(),
            working_dir: "~".into(),
            restart: RestartPolicy::No,
            env: vec![],
        };
        assert_eq!(
            render_unit(&def).unwrap(),
            "[Unit]\n\
             \n\
             [Service]\n\
             Type=simple\n\
             WorkingDirectory=~\n\
             ExecStart=sleep infinity\n\
             Restart=no\n\
             RestartSec=5\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n"
        );
    }

    #[test]
    fn several_variables_keep_their_order() {
        let def = ServiceDef {
            restart: RestartPolicy::Always,
            env: vec![var("Z", "1"), var("A", ""), var("_m2", "x y")],
            ..def()
        };
        let text = render_unit(&def).unwrap();
        assert!(text.contains(
            "Restart=always\nRestartSec=5\n\
             Environment=\"Z=1\"\n\
             Environment=\"A=\"\n\
             Environment=\"_m2=x y\"\n\n[Install]"
        ));
    }

    #[test]
    fn restart_policies_use_systemd_names() {
        for (policy, value, name) in [
            (RestartPolicy::No, "no", "no"),
            (RestartPolicy::OnFailure, "on-failure", "on-failure"),
            (RestartPolicy::Always, "always", "always"),
        ] {
            assert_eq!(policy.value(), value);
            assert_eq!(serde_json::to_value(policy).unwrap(), json!(name));
        }
    }

    #[test]
    fn service_def_deserializes_from_camel_case() {
        let parsed: ServiceDef = serde_json::from_value(json!({
            "name": "bot-discord",
            "description": "Bot Discord",
            "execStart": "/usr/bin/node dist/server.js",
            "workingDir": "/srv/bot-discord",
            "restart": "on-failure",
            "env": [{ "key": "NODE_ENV", "value": "production" }],
        }))
        .unwrap();
        assert_eq!(parsed, def());
    }

    // --- Nom ------------------------------------------------------------

    #[test]
    fn names_follow_the_creation_regex() {
        for valid in ["a", "0", "bot-discord", "a1-b2", "x--y", "a-"] {
            assert_eq!(unit_name(valid).unwrap(), format!("{valid}.service"));
        }
        assert!(unit_name(&"a".repeat(MAX_NAME_CHARS)).is_ok());
        for invalid in [
            "",
            "-a",
            "Bot",
            "bot_discord",
            "bot.discord",
            "bot@1",
            "bot discord",
            "é",
            "bot;rm",
            "../x",
            "bot\n",
        ] {
            assert!(
                matches!(unit_name(invalid), Err(AppError::InvalidInput { .. })),
                "{invalid:?}"
            );
        }
        assert!(unit_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn name_with_suffix_is_refused_with_a_hint() {
        match unit_name("backup.service") {
            Err(AppError::InvalidInput { message }) => assert!(message.contains(".service")),
            other => panic!("{other:?}"),
        }
    }

    // --- Retours à la ligne et caractères de contrôle --------------------

    #[test]
    fn line_breaks_are_refused_in_every_field() {
        for hostile in [
            "x\nExecStartPre=/bin/rm -rf ~",
            "x\rExecStartPre=/bin/true",
            "x\0y",
            "x\ty",
            "x\u{85}y",
        ] {
            let cases = [
                ServiceDef {
                    description: hostile.into(),
                    ..def()
                },
                with_exec(&format!("/bin/echo {hostile}")),
                ServiceDef {
                    working_dir: format!("/srv/{hostile}"),
                    ..def()
                },
                ServiceDef {
                    env: vec![var("A", hostile)],
                    ..def()
                },
                ServiceDef {
                    name: hostile.into(),
                    ..def()
                },
            ];
            for case in cases {
                assert!(
                    matches!(render_unit(&case), Err(AppError::InvalidInput { .. })),
                    "{case:?}"
                );
            }
        }
    }

    #[test]
    fn trailing_backslash_is_refused() {
        // Continuation : la ligne suivante serait jointe à la directive.
        assert!(refused(&with_exec("/bin/echo a \\")).starts_with("Commande"));
        assert!(refused(&ServiceDef {
            description: "C:\\".into(),
            ..def()
        })
        .starts_with("Description"));
        // Dans une valeur entre guillemets, `\` est échappé : la ligne finit par `"`.
        let text = render_unit(&ServiceDef {
            env: vec![var("P", "C:\\")],
            ..def()
        })
        .unwrap();
        assert!(text.contains("Environment=\"P=C:\\\\\"\n"));
    }

    // --- Spécificateurs %, variables $ ----------------------------------

    #[test]
    fn percent_is_escaped_everywhere() {
        assert_eq!(exec_line("/bin/date +%Y-%m-%d"), "/bin/date +%%Y-%%m-%%d");
        assert_eq!(exec_line("/bin/echo %h %%"), "/bin/echo %%h %%%%");
        assert_eq!(working_dir_line("/srv/100%"), "/srv/100%%");
        assert_eq!(working_dir_line("~/a%nb"), "%h/a%%nb");
        let text = render_unit(&ServiceDef {
            description: "%n à 50%".into(),
            env: vec![var("FMT", "%Y")],
            ..def()
        })
        .unwrap();
        assert!(text.contains("Description=%%n à 50%%\n"));
        assert!(text.contains("Environment=\"FMT=%%Y\"\n"));
    }

    #[test]
    fn dollar_is_escaped_in_the_command_only() {
        assert_eq!(
            exec_line("/bin/sh -c 'for i in 1 2; do echo $i ${HOME}; done'"),
            "/bin/sh -c 'for i in 1 2; do echo $$i $${HOME}; done'"
        );
        assert_eq!(exec_line("/bin/echo $$"), "/bin/echo $$$$");
        // `$` n'a aucun sens particulier dans Environment=.
        let text = render_unit(&ServiceDef {
            env: vec![var("PRICE", "5$ $HOME")],
            ..def()
        })
        .unwrap();
        assert!(text.contains("Environment=\"PRICE=5$ $HOME\"\n"));
    }

    // --- Commande ---------------------------------------------------------

    #[test]
    fn quotes_and_escapes_are_kept_for_systemd() {
        assert_eq!(
            exec_line(r#"/bin/echo "deux mots" 'l\'apostrophe' \"x\" \;"#),
            r#"/bin/echo "deux mots" 'l\'apostrophe' \"x\" \;"#
        );
        assert_eq!(
            exec_line(r#""/opt/mon app/run" --x"#),
            r#""/opt/mon app/run" --x"#
        );
    }

    #[test]
    fn special_prefixes_are_refused() {
        for prefix in ["-", "@", ":", "+", "!", "!!", "|", "@-"] {
            let message = refused(&with_exec(&format!("{prefix}/bin/true")));
            assert!(message.contains("préfixe spécial"), "{prefix} : {message}");
        }
        // Après retrait des guillemets, le préfixe reste un préfixe.
        assert!(refused(&with_exec(r#""-/bin/true""#)).contains("préfixe"));
        assert!(refused(&with_exec("'|'echo")).contains("préfixe"));
        // `\x2d` serait « - » pour systemd.
        assert!(refused(&with_exec(r"\x2d/bin/true")).contains("\\"));
    }

    #[test]
    fn program_must_be_absolute_or_a_plain_name() {
        assert_eq!(exec_line("node app.js"), "node app.js");
        assert_eq!(exec_line("/usr/bin/env node"), "/usr/bin/env node");
        assert!(refused(&with_exec("~/bin/bot")).contains("~"));
        for relative in ["./run.sh", "bin/run", "../x"] {
            assert!(
                refused(&with_exec(relative)).contains("chemin absolu"),
                "{relative}"
            );
        }
        assert!(refused(&with_exec("\"\" x")).contains("vide"));
        assert!(refused(&with_exec("   ")).contains("requise"));
    }

    #[test]
    fn lone_semicolon_is_refused_outside_quotes() {
        let message = refused(&with_exec("/bin/echo a ; /bin/rm -rf x"));
        assert!(message.contains("« ; »"), "{message}");
        assert!(refused(&with_exec("/bin/echo a ;")).contains("« ; »"));
        assert!(refused(&with_exec("; /bin/true")).contains("préfixe"));
        // Collé à un mot, échappé ou entre guillemets : un argument ordinaire.
        assert_eq!(exec_line("/bin/echo a; b"), "/bin/echo a; b");
        assert_eq!(exec_line("/bin/echo a \\; b"), "/bin/echo a \\; b");
        assert_eq!(exec_line("/bin/echo ';' \";\""), "/bin/echo ';' \";\"");
        assert_eq!(exec_line("/bin/sh -c 'a ; b'"), "/bin/sh -c 'a ; b'");
    }

    #[test]
    fn unbalanced_quotes_are_refused() {
        for command in [
            "/bin/echo 'a",
            "/bin/echo \"a",
            r#"/bin/echo "a\""#,
            "/bin/echo 'a\"",
        ] {
            assert!(
                refused(&with_exec(command)).contains("guillemet"),
                "{command}"
            );
        }
    }

    #[test]
    fn words_are_split_like_systemd() {
        let words = split_words(r#"/bin/a  "b c"d 'e\'f' g\ h"#).unwrap();
        let texts: Vec<_> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["/bin/a", "b cd", "e'f", "g h"]);
        let raws: Vec<_> = words.iter().map(|w| w.raw).collect();
        assert_eq!(raws, ["/bin/a", r#""b c"d"#, r"'e\'f'", r"g\ h"]);
    }

    #[test]
    fn long_command_is_refused() {
        let long = format!("/bin/echo {}", "x".repeat(MAX_VALUE_BYTES));
        assert!(refused(&with_exec(&long)).contains("octets"));
    }

    // --- Dossier de travail ----------------------------------------------

    #[test]
    fn home_relative_dirs_are_converted() {
        assert_eq!(working_dir_line("~"), "~");
        assert_eq!(working_dir_line("~/"), "~");
        assert_eq!(working_dir_line("~/apps/bot"), "%h/apps/bot");
        assert_eq!(working_dir_line("  /srv/app/  "), "/srv/app/");
        assert_eq!(working_dir_line("/"), "/");
        assert_eq!(working_dir_line("/srv/mon app"), "/srv/mon app");
    }

    #[test]
    fn invalid_working_dirs_are_refused() {
        for dir in [
            "",
            "  ",
            "srv/app",
            "./app",
            "~user/app",
            "-/srv/app",
            "/srv/../etc",
            "/srv/./app",
            "~/..",
            "/srv/a\\b",
            "%h/app",
        ] {
            let message = refused(&ServiceDef {
                working_dir: dir.into(),
                ..def()
            });
            assert!(message.starts_with("Dossier de travail"), "{dir:?}");
        }
    }

    // --- Variables d'environnement --------------------------------------

    #[test]
    fn env_keys_are_validated() {
        for valid in ["A", "_", "_a1", "NODE_ENV", "a"] {
            assert!(is_valid_env_key(valid), "{valid}");
        }
        for invalid in ["", "1A", "A-B", "A B", "A=B", "É", "A$", "A\n"] {
            assert!(!is_valid_env_key(invalid), "{invalid:?}");
            let message = refused(&ServiceDef {
                env: vec![var(invalid, "x")],
                ..def()
            });
            assert!(message.contains("invalide"), "{invalid:?}");
        }
    }

    #[test]
    fn env_values_escape_quotes_and_backslashes() {
        let text = render_unit(&ServiceDef {
            env: vec![var("V", r#"say "hi" \n C:\dir 'x' 100%"#)],
            ..def()
        })
        .unwrap();
        assert!(text.contains(r#"Environment="V=say \"hi\" \\n C:\\dir 'x' 100%%""#));
    }

    #[test]
    fn duplicate_and_excess_env_vars_are_refused() {
        assert!(refused(&ServiceDef {
            env: vec![var("A", "1"), var("A", "2")],
            ..def()
        })
        .contains("double"));
        let many = (0..=MAX_ENV_VARS)
            .map(|i| var(&format!("V{i}"), "x"))
            .collect();
        assert!(refused(&ServiceDef { env: many, ..def() }).contains("au plus"));
    }

    mod shared {
        use super::*;
        use crate::shared_cases::{check_valid, shared_cases};

        fn renders(def: ServiceDef) -> bool {
            render_unit(&def).is_ok()
        }

        #[test]
        fn name() {
            check_valid(shared_cases!("service-name.json"), |name: &String| {
                unit_name(name).is_ok()
            });
        }

        #[test]
        fn description() {
            check_valid(shared_cases!("service-description.json"), |d: &String| {
                renders(ServiceDef {
                    description: d.clone(),
                    ..def()
                })
            });
        }

        #[test]
        fn working_dir() {
            check_valid(shared_cases!("service-working-dir.json"), |w: &String| {
                renders(ServiceDef {
                    working_dir: w.clone(),
                    ..def()
                })
            });
        }

        #[test]
        fn exec_start() {
            check_valid(shared_cases!("service-exec-start.json"), |c: &String| {
                renders(ServiceDef {
                    exec_start: c.clone(),
                    ..def()
                })
            });
        }

        #[test]
        fn env() {
            check_valid(shared_cases!("service-env.json"), |env: &Vec<EnvVar>| {
                renders(ServiceDef {
                    env: env.clone(),
                    ..def()
                })
            });
        }
    }
}
