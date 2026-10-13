import { ref } from "vue";
import { defineStore } from "pinia";

export const useConnectionStore = defineStore("connection", () => {
  const isConnected = ref(false);

  return { isConnected };
});
