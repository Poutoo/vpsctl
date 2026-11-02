/**
 * docs/tests-jalon-11.md, section 2 : ce ne sont pas des scénarios
 * d'interface. Ils restent listés ici pour apparaître dans le rapport, mais
 * se lancent par leurs propres commandes (et par la CI).
 */
const REASON =
  "hors E2E : commande à lancer telle quelle (CI : .github/workflows/ci.yml)";

describe("Jalon 11 · 2. Tests automatisés", () => {
  it.skip(`npm ci puis npm test : tous les tests passent — ${REASON}`);
  it.skip(`cd src-tauri && cargo test : tous les tests passent — ${REASON}`);
  it.skip(
    `lint, typecheck, prettier, cargo fmt, clippy : rien à signaler — ${REASON}`,
  );
  it.skip(`npm run tauri build réussit — ${REASON}`);
  it.skip(
    "Linux : mêmes commandes — hors périmètre : tests E2E sous Windows uniquement",
  );
});
