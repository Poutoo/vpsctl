import { ref } from "vue";
import { defineStore } from "pinia";
import * as api from "../api/scripts";
import type { Script } from "../bindings/Script";

/** Scripts enregistrés localement. `lastRun` est relu après chaque exécution. */
export const useScriptsStore = defineStore("scripts", () => {
  const scripts = ref<Script[]>([]);
  const loaded = ref(false);

  function byId(id: string | null): Script | null {
    return scripts.value.find((s) => s.id === id) ?? null;
  }

  async function load() {
    scripts.value = await api.listScripts();
    loaded.value = true;
  }

  async function save(script: Script): Promise<Script> {
    const saved = await api.saveScript(script);
    await load();
    return saved;
  }

  async function remove(id: string) {
    await api.deleteScript(id);
    await load();
  }

  return { scripts, loaded, byId, load, save, remove };
});
