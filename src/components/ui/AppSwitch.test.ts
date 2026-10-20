import { mount } from "@vue/test-utils";
import { TooltipProvider } from "reka-ui";
import { describe, expect, it } from "vitest";
import { defineComponent, h, ref } from "vue";
import AppSwitch from "./AppSwitch.vue";

function mountSwitch(props: Record<string, unknown>, initial = false) {
  const value = ref(initial);
  const wrapper = mount(
    defineComponent({
      setup: () => () =>
        h(TooltipProvider, null, () =>
          h(AppSwitch, {
            label: "Démarrage automatique de bot.service",
            hideLabel: true,
            showState: true,
            ...props,
            modelValue: value.value,
            "onUpdate:modelValue": (next: boolean) => (value.value = next),
          }),
        ),
    }),
  );
  return { wrapper, value, button: wrapper.get('[role="switch"]') };
}

describe("interrupteur", () => {
  it("bascule au clic et affiche l'état", async () => {
    const { wrapper, value, button } = mountSwitch({});
    expect(wrapper.text()).toContain("Désactivé");
    await button.trigger("click");
    expect(value.value).toBe(true);
    expect(button.attributes("aria-checked")).toBe("true");
    expect(wrapper.text()).toContain("Activé");
  });

  it("ne bascule pas pendant un changement, et l'annonce au participe", async () => {
    const { wrapper, value, button } = mountSwitch({ busyLabel: "Activation" });
    await button.trigger("click");
    expect(value.value).toBe(false);
    expect(button.attributes("aria-busy")).toBe("true");
    expect(wrapper.text()).toContain("Activation");
  });

  it("reste focalisable mais inerte quand il n'est pas modifiable", async () => {
    const { wrapper, value, button } = mountSwitch({
      locked: "Statique",
      help: "Pas de section [Install].",
    });
    await button.trigger("click");
    expect(value.value).toBe(false);
    expect(button.attributes("aria-disabled")).toBe("true");
    expect(button.attributes("disabled")).toBeUndefined();
    expect(button.attributes("aria-label")).toContain(
      "Pas de section [Install].",
    );
    expect(wrapper.text()).toContain("Statique");
  });

  it("donne la raison d'une indisponibilité (liaison perdue)", async () => {
    const { value, button } = mountSwitch(
      { unavailable: "liaison perdue" },
      true,
    );
    await button.trigger("click");
    expect(value.value).toBe(true);
    expect(button.attributes("aria-label")).toContain("liaison perdue");
  });
});
