import { globalIgnores } from "eslint/config";
import {
  defineConfigWithVueTs,
  vueTsConfigs,
} from "@vue/eslint-config-typescript";
import pluginVue from "eslint-plugin-vue";
import skipFormatting from "@vue/eslint-config-prettier/skip-formatting";

export default defineConfigWithVueTs(
  {
    name: "app/files-to-lint",
    files: ["**/*.{js,ts,mts,vue}"],
  },

  globalIgnores(["dist/**", "src-tauri/**", "src/bindings/**"]),

  pluginVue.configs["flat/essential"],
  vueTsConfigs.recommended,

  // CLAUDE.md : aucun appel à invoke() en dehors de src/api/.
  {
    name: "app/invoke-only-in-api",
    files: ["src/**/*.{ts,vue}"],
    ignores: ["src/api/**"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "@tauri-apps/api/core",
              importNames: ["invoke"],
              message: "invoke() n'est autorisé que dans src/api/.",
            },
          ],
        },
      ],
    },
  },

  // Le formatage relève de Prettier, pas d'ESLint.
  skipFormatting,
);
