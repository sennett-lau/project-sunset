import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { App } from "../../App";
import { createFakeClient } from "../../testing/fake-client";

afterEach(cleanup);

test("add a local provider, test a model and use it for the tutor", async () => {
  const client = createFakeClient();
  render(<App client={client} productName="Test" />);

  fireEvent.click(await screen.findByRole("button", { name: /Models/ }));
  const panel = screen.getByRole("complementary", { name: "Models" });
  await within(panel).findByText(/Every role runs on this Mac/);

  fireEvent.click(within(panel).getByRole("button", { name: "Add provider" }));
  fireEvent.click(within(panel).getByRole("button", { name: "Ollama" }));
  fireEvent.change(within(panel).getByLabelText(/API key/), { target: { value: "sk-local" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Save" }));
  await within(panel).findByText("http://localhost:11434/v1");
  expect(within(panel).getByText("API key saved in Keychain")).toBeTruthy();
  expect(panel.textContent).not.toContain("sk-local");

  const providerSelect = within(panel).getByLabelText("Tutor provider");
  const ollama = (await client.modelSettings()).providers.find((p) => p.name === "Ollama")!;
  fireEvent.change(providerSelect, { target: { value: ollama.id } });
  fireEvent.change(within(panel).getByLabelText("Tutor model"), { target: { value: "qwen3:8b" } });

  fireEvent.click(within(panel).getAllByRole("button", { name: "Test connection" })[0]!);
  await within(panel).findByText("Basic request: OK");

  fireEvent.click(within(panel).getAllByRole("button", { name: "Use this model" })[0]!);
  await waitFor(async () => {
    const tutor = (await client.modelSettings()).roles.find((r) => r.role === "tutor")!;
    expect(tutor.model).toBe("qwen3:8b");
  });
  await screen.findByText("qwen3:8b", { selector: "header span" });
});
