# Cas partagés Rust / TypeScript

Règles appliquées à l'identique par le backend (qui fait foi) et par la validation instantanée de l'interface. Chaque fichier est lu par `cargo test` et par `npm test` : une divergence fait échouer l'un des deux.

```json
{
  "rule": "règle couverte",
  "rust": "fonction Rust testée",
  "ts": "fonction TypeScript testée",
  "cases": [{ "input": "…", "valid": true, "why": "justification" }]
}
```

- `valid` : verdict attendu d'une validation (les messages ne sont pas comparés) ; `expected` : résultat attendu d'une transformation.
- Une chaîne longue s'écrit `{ "repeat": "é", "times": 128, "before": "/", "after": "" }` (`before` et `after` facultatifs), n'importe où dans `input`.
- Seules les règles présentes des deux côtés figurent ici (inventaire : `docs/03-architecture.md`, « Cas partagés Rust / TypeScript »).
