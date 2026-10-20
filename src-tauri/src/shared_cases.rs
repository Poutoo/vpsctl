//! Cas partagés avec l'interface (`tests/shared/`, voir son README) : même
//! entrée, même verdict des deux côtés.

use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

/// Contenu d'un fichier de `tests/shared/`, inclus à la compilation.
macro_rules! shared_cases {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/shared/",
            $file
        ))
    };
}
pub(crate) use shared_cases;

#[derive(Deserialize)]
struct CaseFile {
    cases: Vec<RawCase>,
}

#[derive(Deserialize)]
struct RawCase {
    input: Value,
    valid: Option<bool>,
    expected: Option<Value>,
    why: String,
}

/// `{ "repeat": s, "times": n, "before"?: p, "after"?: q }` → chaîne.
fn expand(value: Value) -> Value {
    match value {
        Value::Object(map) if map.contains_key("repeat") => {
            let text = |key: &str| map.get(key).and_then(Value::as_str).unwrap_or("");
            let times = map.get("times").and_then(Value::as_u64).unwrap_or(0) as usize;
            Value::String(format!(
                "{}{}{}",
                text("before"),
                text("repeat").repeat(times),
                text("after")
            ))
        }
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, expand(v))).collect()),
        Value::Array(items) => Value::Array(items.into_iter().map(expand).collect()),
        other => other,
    }
}

fn cases(json: &str) -> Vec<RawCase> {
    let file: CaseFile = serde_json::from_str(json).expect("fichier de cas partagés illisible");
    file.cases
        .into_iter()
        .map(|case| RawCase {
            input: expand(case.input),
            ..case
        })
        .collect()
}

fn input<I: DeserializeOwned>(case: &RawCase) -> I {
    serde_json::from_value(case.input.clone())
        .unwrap_or_else(|e| panic!("entrée illisible ({}) : {e}", case.why))
}

fn report(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "cas partagés en échec :\n{}",
        failures.join("\n")
    );
}

/// Vérifie le verdict (`valid`) de chaque cas.
pub fn check_valid<I: DeserializeOwned + Debug>(json: &str, accepts: impl Fn(&I) -> bool) {
    let failures = cases(json)
        .iter()
        .filter_map(|case| {
            let expected = case.valid.expect("champ `valid` attendu");
            let value: I = input(case);
            let got = accepts(&value);
            (got != expected).then(|| {
                format!(
                    "- {value:?} : attendu {}, obtenu {} ({})",
                    verdict(expected),
                    verdict(got),
                    case.why
                )
            })
        })
        .collect();
    report(failures);
}

/// Vérifie le résultat (`expected`) de chaque cas.
pub fn check_expected<I: DeserializeOwned + Debug, O: serde::Serialize>(
    json: &str,
    transform: impl Fn(&I) -> O,
) {
    let failures = cases(json)
        .iter()
        .filter_map(|case| {
            let expected = case.expected.clone().unwrap_or(Value::Null);
            let value: I = input(case);
            let got = serde_json::to_value(transform(&value)).expect("résultat sérialisable");
            (got != expected).then(|| {
                format!(
                    "- {value:?} : attendu {expected}, obtenu {got} ({})",
                    case.why
                )
            })
        })
        .collect();
    report(failures);
}

fn verdict(valid: bool) -> &'static str {
    if valid {
        "valide"
    } else {
        "invalide"
    }
}
