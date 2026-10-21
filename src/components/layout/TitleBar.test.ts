import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { TooltipProvider } from "reka-ui";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, h } from "vue";
import * as windowApi from "../../api/window";
import TitleBar from "./TitleBar.vue";

vi.mock("../../api/window");
vi.mock("../../api/connection");

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(windowApi.watchMaximized).mockResolvedValue(() => {});
});

function mountBar() {
  return mount(
    defineComponent({
      setup: () => () => h(TooltipProvider, null, () => h(TitleBar)),
    }),
  );
}

describe("barre de titre", () => {
  it("affiche le symbole, décoratif, à côté du mot Atlas", () => {
    const wrapper = mountBar();
    const symbol = wrapper.get("img.symbol");
    expect(symbol.attributes("alt")).toBe("");
    expect(symbol.attributes("src")).toContain("atlas-symbole");
    expect(symbol.attributes("draggable")).toBe("false");
    expect(wrapper.get(".name").text()).toBe("Atlas");
  });

  it("garde « Contrôle VPS » comme élément distinct, masquable en fenêtre étroite", () => {
    const wrapper = mountBar();
    expect(wrapper.get(".product").text()).toBe("Contrôle VPS");
  });

  it("garde les boutons de fenêtre avec leur nom accessible", () => {
    const wrapper = mountBar();
    for (const label of ["Réduire", "Agrandir", "Fermer"]) {
      expect(wrapper.find(`button[aria-label="${label}"]`).exists()).toBe(true);
    }
  });
});
