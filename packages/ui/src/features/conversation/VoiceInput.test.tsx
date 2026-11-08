import type { SunsetClient } from "@sunset/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { ClientProvider } from "../../client-context";
import { createFakeClient } from "../../testing/fake-client";
import { Conversation } from "./Conversation";
import type { ConversationState } from "./useConversation";

afterEach(cleanup);

async function speechClient() {
  const client = createFakeClient();
  const settings = await client.modelSettings();
  const provider = await client.saveProvider(null, { name: "My speech server", baseUrl: "http://localhost:8180/v1", apiKey: null });
  client.modelSettings = async () => ({
    ...settings,
    providers: [...settings.providers, provider],
    roles: [...settings.roles, { role: "stt", providerId: provider.id, model: "qwen3-asr", capabilities: { streaming: "needs_test", jsonSchema: "needs_test", toolCalls: "needs_test", probedAt: null } }],
  });
  return Object.assign(client, {
    startRecording: vi.fn(async () => {}),
    stopRecording: vi.fn(async () => "What is a latent space?"),
    cancelRecording: vi.fn(async () => {}),
    recordingStatus: vi.fn(async () => ({ recording: true, elapsedMs: 1000, autoStopped: false, error: null })),
  });
}

function mount(client: SunsetClient) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const state: ConversationState = { messages: [], sessionId: null, busy: false, loadingHistory: false, historyError: null, resume: vi.fn(async () => {}), newConversation: vi.fn(), send: vi.fn(async () => {}), onEvent: () => {} };
  const tree = () => <ClientProvider value={client}><QueryClientProvider client={queryClient}><Conversation state={state} /></QueryClientProvider></ClientProvider>;
  const view = render(tree());
  return { state, view, queryClient, rerender: () => view.rerender(tree()) };
}

function draft() { return screen.getByRole("textbox", { name: "Your next question" }) as HTMLTextAreaElement; }

async function record() {
  const button = screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  fireEvent.click(button);
  await screen.findByRole("button", { name: "Stop & transcribe" });
}

test("recording locks conversation changes and appends a reviewed transcript without sending", async () => {
  const client = await speechClient();
  const { state } = mount(client);
  fireEvent.change(draft(), { target: { value: "Please explain:" } });
  await record();
  expect((screen.getByRole("button", { name: "Send" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "New conversation" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("combobox", { name: "Conversation history" }) as HTMLSelectElement).disabled).toBe(true);
  fireEvent.submit(draft().closest("form")!);
  expect(state.send).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Stop & transcribe" }));
  await waitFor(() => expect(draft().value).toBe("Please explain:\nWhat is a latent space?"));
  expect(state.send).not.toHaveBeenCalled();
  expect(client.startRecording).toHaveBeenCalledTimes(1);
  expect(client.stopRecording).toHaveBeenCalledTimes(1);
});

test("cancel during microphone setup waits for acknowledgement then stops late capture", async () => {
  const client = await speechClient();
  let started!: () => void;
  client.startRecording.mockImplementation(() => new Promise<void>((resolve) => { started = resolve; }));
  mount(client);
  const button = screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  fireEvent.click(button);
  fireEvent.click(screen.getByRole("button", { name: "Cancel recording" }));
  expect(client.cancelRecording).not.toHaveBeenCalled();
  await act(async () => { started(); });
  await waitFor(() => expect(client.cancelRecording).toHaveBeenCalledTimes(1));
  expect(client.stopRecording).not.toHaveBeenCalled();
  expect(draft().value).toBe("");
});

test("cancelled transcription cannot append a late result or clear a typed draft", async () => {
  const client = await speechClient();
  let finish!: (text: string) => void;
  client.stopRecording.mockImplementation(() => new Promise<string>((resolve) => { finish = resolve; }));
  mount(client);
  fireEvent.change(draft(), { target: { value: "Keep this" } });
  await record();
  fireEvent.click(screen.getByRole("button", { name: "Stop & transcribe" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel recording" }));
  await waitFor(() => expect(client.cancelRecording).toHaveBeenCalledTimes(1));
  await act(async () => { finish("stale text"); });
  expect(draft().value).toBe("Keep this");
});

test("failed transcription preserves the draft and allows another recording", async () => {
  const client = await speechClient();
  client.stopRecording.mockRejectedValueOnce(new Error("Speech server unavailable"));
  mount(client);
  fireEvent.change(draft(), { target: { value: "Existing question" } });
  await record();
  fireEvent.click(screen.getByRole("button", { name: "Stop & transcribe" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Speech server unavailable");
  expect(draft().value).toBe("Existing question");
  expect((screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement).disabled).toBe(false);
});

test("two-minute automatic stop transcribes once and never sends", async () => {
  const client = await speechClient();
  client.recordingStatus.mockResolvedValue({ recording: false, elapsedMs: 120000, autoStopped: true, error: null });
  const { state } = mount(client);
  const button = screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  fireEvent.click(button);
  await waitFor(() => expect(draft().value).toBe("What is a latent space?"));
  expect(client.stopRecording).toHaveBeenCalledTimes(1);
  expect(state.send).not.toHaveBeenCalled();
});

test("changed speech destination cancels capture and discards an old transcript", async () => {
  const client = await speechClient();
  let finish!: (text: string) => void;
  client.stopRecording.mockImplementation(() => new Promise<string>((resolve) => { finish = resolve; }));
  const { queryClient } = mount(client);
  await record();
  fireEvent.click(screen.getByRole("button", { name: "Stop & transcribe" }));
  const settings = await client.modelSettings();
  await act(async () => { queryClient.setQueryData(["models"], { ...settings, providers: settings.providers.map((p) => p.kind === "demo" ? p : { ...p, baseUrl: "https://different.example/v1" }) }); });
  await waitFor(() => expect(client.cancelRecording).toHaveBeenCalledTimes(1));
  await act(async () => { finish("wrong destination text"); });
  expect(draft().value).toBe("");
});

test("external session switch discards transcription from the earlier conversation", async () => {
  const client = await speechClient();
  let finish!: (text: string) => void;
  client.stopRecording.mockImplementation(() => new Promise<string>((resolve) => { finish = resolve; }));
  const { state, rerender } = mount(client);
  await record();
  fireEvent.click(screen.getByRole("button", { name: "Stop & transcribe" }));
  state.sessionId = "another-session";
  rerender();
  await waitFor(() => expect(client.cancelRecording).toHaveBeenCalledTimes(1));
  await act(async () => { finish("earlier question"); });
  expect(draft().value).toBe("");
});

test("denied microphone setup remains recoverable without changing typed text", async () => {
  const client = await speechClient();
  client.startRecording.mockRejectedValueOnce(new Error("Microphone permission denied"));
  mount(client);
  fireEvent.change(draft(), { target: { value: "A typed question" } });
  const button = screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  fireEvent.click(button);
  expect((await screen.findByRole("alert")).textContent).toContain("Microphone permission denied");
  expect(draft().value).toBe("A typed question");
  expect((screen.getByRole("button", { name: "Record a question" }) as HTMLButtonElement).disabled).toBe(false);
  expect(client.stopRecording).not.toHaveBeenCalled();
});

test("unmount stops an active microphone and a failed cancellation stays visibly locked", async () => {
  const client = await speechClient();
  client.cancelRecording.mockRejectedValueOnce(new Error("Device did not acknowledge stop"));
  const { view } = mount(client);
  await record();
  fireEvent.click(screen.getByRole("button", { name: "Cancel recording" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Could not confirm recording stopped");
  expect(screen.getByText("Microphone stop unconfirmed")).toBeTruthy();
  expect((screen.getByRole("button", { name: "New conversation" }) as HTMLButtonElement).disabled).toBe(true);
  view.unmount();
  await waitFor(() => expect(client.cancelRecording).toHaveBeenCalledTimes(2));
});
