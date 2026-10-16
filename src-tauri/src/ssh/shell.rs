//! Échappement des arguments insérés dans une commande shell distante.
//!
//! Règle : un argument venant de l'interface est d'abord validé par une
//! expression stricte (dans `docker` ou `systemd`), **puis** échappé ici,
//! même quand la validation rend l'échappement superflu.

/// Entoure `arg` d'apostrophes : le shell n'interprète rien entre elles
/// (ni `$`, ni `` ` ``, ni `;`, ni retour à la ligne). Une apostrophe de
/// l'argument devient `'\''` (fermeture, apostrophe échappée, réouverture).
pub fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn plain_argument_is_wrapped_in_single_quotes() {
        assert_eq!(quote("bot.service"), "'bot.service'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn shell_syntax_is_kept_inside_the_quotes() {
        assert_eq!(quote("x; rm -rf ~"), "'x; rm -rf ~'");
        assert_eq!(quote("$(id)"), "'$(id)'");
        assert_eq!(quote("`id`"), "'`id`'");
        assert_eq!(quote("a\"b"), "'a\"b'");
        assert_eq!(quote("a\nb"), "'a\nb'");
    }

    #[test]
    fn single_quote_is_closed_escaped_and_reopened() {
        assert_eq!(quote("a'b"), r"'a'\''b'");
        assert_eq!(quote("'; rm -rf ~; '"), r"''\''; rm -rf ~; '\'''");
    }
}
