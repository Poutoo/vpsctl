import { computed, ref } from "vue";
import { defineStore } from "pinia";
import * as api from "../api/connection";
import type { Profile } from "../bindings/Profile";

export const useProfilesStore = defineStore("profiles", () => {
  const profiles = ref<Profile[]>([]);
  const loaded = ref(false);

  /** Profil présélectionné : celui par défaut, sinon le premier. */
  const preferred = computed<Profile | null>(
    () => profiles.value.find((p) => p.isDefault) ?? profiles.value[0] ?? null,
  );

  function byId(id: string | null): Profile | null {
    return profiles.value.find((p) => p.id === id) ?? null;
  }

  async function load() {
    profiles.value = await api.listProfiles();
    loaded.value = true;
  }

  async function ensureLoaded() {
    if (!loaded.value) await load();
  }

  async function save(profile: Profile): Promise<Profile> {
    const saved = await api.saveProfile(profile);
    await load();
    return saved;
  }

  async function remove(id: string) {
    await api.deleteProfile(id);
    await load();
  }

  async function setDefault(id: string) {
    const profile = byId(id);
    if (profile) await save({ ...profile, isDefault: true });
  }

  return {
    profiles,
    loaded,
    preferred,
    byId,
    load,
    ensureLoaded,
    save,
    remove,
    setDefault,
  };
});
