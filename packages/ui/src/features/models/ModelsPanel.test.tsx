import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
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

test("connection testing locks the selected provider and model until results arrive", async () => {
  const client = createFakeClient();
  const original = client.testConnection;
  let finish!: () => void;
  client.testConnection = async (provider, model) => {
    await new Promise<void>((resolve) => { finish = resolve; });
    return original(provider, model);
  };
  render(<App client={client} productName="Test" />);
  fireEvent.click(await screen.findByRole("button", { name: /Models/ }));
  const panel = screen.getByRole("complementary", { name: "Models" });
  fireEvent.click((await within(panel).findAllByRole("button", { name: "Test connection" }))[0]!);
  await waitFor(() => expect((within(panel).getByLabelText("Tutor model") as HTMLInputElement).disabled).toBe(true));
  expect((within(panel).getByLabelText("Tutor provider") as HTMLSelectElement).disabled).toBe(true);
  finish();
  await within(panel).findByText("Basic request: OK");
  expect((within(panel).getByLabelText("Tutor model") as HTMLInputElement).disabled).toBe(false);
});

test("speech is an optional separate role and never receives a chat capability probe", async () => {
  const client = createFakeClient();
  const provider = await client.saveProvider(null, { name: "Speech server", baseUrl: "http://localhost:8180/v1", apiKey: null });
  client.testConnection = vi.fn(client.testConnection);
  render(<App client={client} productName="Test" />);
  fireEvent.click(await screen.findByRole("button", { name: /Models/ }));
  const speech = await screen.findByRole("region", { name: "Speech recognition" });
  expect(within(speech).queryByRole("option", { name: /Demo/ })).toBeNull();
  expect(within(speech).queryByRole("button", { name: "Test connection" })).toBeNull();
  fireEvent.change(within(speech).getByLabelText("Speech recognition provider"), { target: { value: provider.id } });
  fireEvent.change(within(speech).getByLabelText("Speech recognition model"), { target: { value: "qwen3-asr" } });
  fireEvent.click(within(speech).getByRole("button", { name: "Use for speech" }));
  await waitFor(async () => expect((await client.modelSettings()).roles.find((r) => r.role === "stt")?.model).toBe("qwen3-asr"));
  expect(client.testConnection).not.toHaveBeenCalled();
  await waitFor(() => expect((screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement).disabled).toBe(false));
});
