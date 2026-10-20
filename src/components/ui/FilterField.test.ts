import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { defineComponent, h, ref } from "vue";
import { installFindShortcut } from "../../findShortcut";
import FilterField from "./FilterField.vue";

let uninstall: () => void = () => {};

beforeEach(() => {
  uninstall = installFindShortcut();
});

afterEach(() => {
  uninstall();
  document.body.innerHTML = "";
});

function mountField(initial = "", shortcutEnabled = true) {
  const query = ref(initial);
  const left = ref(0);
  const wrapper = mount(
    defineComponent({
      setup: () => () =>
        h(FilterField, {
          label: "Filtrer les tableaux",
          shortcutEnabled,
          modelValue: query.value,
          "onUpdate:modelValue": (value: string) => (query.value = value),
          onLeave: () => left.value++,
        }),
    }),
    { attachTo: document.body },
  );
  return { wrapper, query, left, input: wrapper.get("input") };
}

function pressCtrlF(): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    key: "f",
    ctrlKey: true,
    bubbles: true,
    cancelable: true,
  });
  document.body.dispatchEvent(event);
  return event;
}

describe("Ctrl+F", () => {
  it("place le curseur dans le filtre et sélectionne son texte", async () => {
    const { input } = mountField("urban");
    const event = pressCtrlF();
    expect(event.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(input.element);
    const element = input.element as HTMLInputElement;
    expect([element.selectionStart, element.selectionEnd]).toEqual([0, 5]);
  });

  it("est intercepté même sans filtre à l'écran", () => {
    expect(pressCtrlF().defaultPrevented).toBe(true);
  });

  it("laisse la main à une fenêtre modale ouverte", () => {
    const { input } = mountField();
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "alertdialog");
    dialog.dataset.state = "open";
    document.body.append(dialog);
    expect(pressCtrlF().defaultPrevented).toBe(true);
    expect(document.activeElement).not.toBe(input.element);
  });

  it("ne prend pas le focus pendant une saisie en cours", () => {
    const { input } = mountField("", false);
    pressCtrlF();
    expect(document.activeElement).not.toBe(input.element);
  });

  it("ne réagit pas à Ctrl+Maj+F", () => {
    mountField();
    const event = new KeyboardEvent("keydown", {
      key: "F",
      ctrlKey: true,
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    document.body.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });

  it("n'est plus relié au filtre une fois démonté", () => {
    const { wrapper } = mountField();
    wrapper.unmount();
    expect(pressCtrlF().defaultPrevented).toBe(true);
  });
});

describe("Échap", () => {
  it("vide le filtre, puis rend le focus à la liste", async () => {
    const { input, query, left } = mountField("nginx");
    await input.trigger("keydown", { key: "Escape" });
    expect(query.value).toBe("");
    expect(left.value).toBe(0);
    await input.trigger("keydown", { key: "Escape" });
    expect(left.value).toBe(1);
  });
});

describe("affichage", () => {
  it("rappelle le raccourci au repos et propose d'effacer une fois rempli", async () => {
    const { wrapper, query } = mountField();
    expect(wrapper.find(".keys").exists()).toBe(true);
    query.value = "a";
    await wrapper.vm.$nextTick();
    expect(wrapper.find(".keys").exists()).toBe(false);
    await wrapper.get('[aria-label="Effacer le filtre"]').trigger("click");
    expect(query.value).toBe("");
  });
});
