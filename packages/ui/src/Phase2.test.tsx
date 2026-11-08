import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { AppEvent, SendMessageResult } from "@sunset/client";
import { App } from "./App";
import { createFakeClient } from "./testing/fake-client";

afterEach(cleanup);

async function seeded() {
  const client = createFakeClient();
  await client.sendMessage(null, "What is a VAE?");
  await client.approveProposal((await client.pendingProposals())[0]!.id);
  return client;
}

test("wiki searches aliases through the backend, debounces, and refreshes on knowledge changes", async () => {
  const client = await seeded();
  const search = vi.spyOn(client, "searchKnowledge");
  render(<App client={client} productName="Test" />);
  const input = await screen.findByRole("searchbox", { name: "Search knowledge" });
  fireEvent.change(input, { target: { value: "V" } });
  fireEvent.change(input, { target: { value: "VA" } });
  fireEvent.change(input, { target: { value: "VAE" } });
  expect(search).not.toHaveBeenCalled();
  await waitFor(() => expect(search).toHaveBeenCalledExactlyOnceWith("VAE", 50));
  expect(await screen.findByRole("button", { name: "Variational Autoencoder" })).toBeTruthy();
  await act(async () => { await client.approveProposal((await client.recentProposals(1))[0]!.id); });
  await waitFor(() => expect(search).toHaveBeenCalledTimes(2));
  fireEvent.change(input, { target: { value: "" } });
  expect(screen.getByRole("button", { name: "Variational Autoencoder" })).toBeTruthy();
});

test("wiki handles fulltext results, pending search, failure, and retry", async () => {
  const client = await seeded();
  const concepts = await client.listConcepts();
  let resolve!: (value: typeof concepts) => void;
  const search = vi.spyOn(client, "searchKnowledge")
    .mockImplementationOnce(() => new Promise((r) => { resolve = r; }))
    .mockRejectedValueOnce(new Error("Index unavailable"))
    .mockResolvedValue(concepts);
  render(<App client={client} productName="Test" />);
  fireEvent.change(await screen.findByRole("searchbox"), { target: { value: "distribution" } });
  await waitFor(() => expect(search).toHaveBeenCalledWith("distribution", 50));
  expect(screen.getByText("Searching knowledge…")).toBeTruthy();
  await act(async () => resolve(concepts));
  expect(await screen.findByRole("button", { name: "Variational Autoencoder" })).toBeTruthy();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "encoder" } });
  await screen.findByText(/Search failed/);
  expect(screen.queryByText("No matching articles.")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Retry search" }));
  expect(await screen.findByRole("button", { name: "Variational Autoencoder" })).toBeTruthy();
});

test("new conversation retains prior transcripts, resumes them, and appends to the selected session", async () => {
  const client = createFakeClient();
  render(<App client={client} productName="Test" />);
  const ask = async (text: string) => {
    fireEvent.change(screen.getByRole("textbox", { name: "Your next question" }), { target: { value: text } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await screen.findByText(text);
    await waitFor(() => expect((screen.getByRole("button", { name: "New conversation" }) as HTMLButtonElement).disabled).toBe(false));
  };
  await ask("First question");
  const first = (await client.listSessions(100))[0]!;
  fireEvent.click(screen.getByRole("button", { name: "New conversation" }));
  expect(screen.queryByText("First question")).toBeNull();
  await ask("Second question");
  fireEvent.change(screen.getByRole("combobox", { name: "Conversation history" }), { target: { value: first.id } });
  await screen.findByText("First question");
  expect(screen.queryByText("Second question")).toBeNull();
  await ask("Follow up");
  expect((await client.listSessions(100)).length).toBe(2);
  expect((await client.listTurns(first.id)).map((t) => t.text)).toContain("Follow up");
});

test("early events are buffered by returned session; unrelated events cannot contaminate a new conversation", async () => {
  const client = createFakeClient();
  let listener!: (event: AppEvent) => void;
  client.onEvent = (fn) => { listener = fn; return () => {}; };
  let resolve!: (result: SendMessageResult) => void;
  client.sendMessage = vi.fn(() => new Promise<SendMessageResult>((r) => { resolve = r; }));
  render(<App client={client} productName="Test" />);
  act(() => listener({ type: "tutor_delta", sessionId: "stranger", turnId: "alien", text: "Foreign answer" }));
  expect(screen.queryByText("Foreign answer")).toBeNull();
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "My question" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect((screen.getByRole("button", { name: "New conversation" }) as HTMLButtonElement).disabled).toBe(true);
  const userTurn = { id: "user", sessionId: "mine", speaker: "user" as const, text: "My question", createdAt: new Date().toISOString() };
  await act(async () => {
    listener({ type: "tutor_delta", sessionId: "stranger", turnId: "alien", text: "Foreign answer" });
    listener({ type: "tutor_delta", sessionId: "mine", turnId: "answer", text: "Partial" });
    listener({ type: "turn_finalized", turn: { ...userTurn, id: "answer", speaker: "tutor", text: "Complete answer" } });
    resolve({ sessionId: "mine", userTurn, tutorTurnId: "answer" });
  });
  await screen.findByText("Complete answer");
  expect(screen.queryByText("Foreign answer")).toBeNull();
  expect(screen.queryByText("Partial")).toBeNull();
  expect(screen.getAllByText("My question")).toHaveLength(1);
  expect((screen.getByRole("button", { name: "New conversation" }) as HTMLButtonElement).disabled).toBe(false);
});

test("history load failure preserves the current transcript and permits retry", async () => {
  const client = createFakeClient();
  const old = await client.sendMessage(null, "Saved question");
  const turns = client.listTurns.bind(client);
  client.listTurns = vi.fn().mockRejectedValueOnce(new Error("Database busy")).mockImplementation(turns);
  render(<App client={client} productName="Test" />);
  await waitFor(() => expect(screen.getByRole("combobox", { name: "Conversation history" }).querySelectorAll("option")).toHaveLength(2));
  fireEvent.change(screen.getByRole("combobox", { name: "Conversation history" }), { target: { value: old.sessionId } });
  await screen.findByText(/Could not open conversation: Error: Database busy/);
  expect(screen.getByText("Make room for", { exact: false })).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "Conversation history" }), { target: { value: old.sessionId } });
  await screen.findByText("Saved question");
});

test("Library invokes only native public export, shows its path, and recovers from export and sync failures", async () => {
  const client = createFakeClient();
  const exportFolder = vi.spyOn(client, "exportPublicToFolder")
    .mockRejectedValueOnce(new Error("Disk full"))
    .mockResolvedValue("/exports/public-fresh");
  const rawExport = vi.spyOn(client, "exportPublic");
  const turns = vi.spyOn(client, "listTurns");
  const page = vi.spyOn(client, "conceptPage");
  client.vaultStatus = async () => ({ path: "/vault", conflicts: ["concepts/edited.md"], error: "Write failed" });
  client.syncVault = vi.fn().mockRejectedValueOnce(new Error("Permission denied"))
    .mockResolvedValue({ path: "/vault", conflicts: [], error: null });
  render(<App client={client} productName="Test" />);
  fireEvent.click(screen.getByRole("button", { name: "Library" }));
  const panel = within(screen.getByRole("complementary", { name: "Library" }));
  await panel.findByText("concepts/edited.md");
  expect(panel.getByText("/vault")).toBeTruthy();
  expect(panel.getByText(/Conversations, private learning notes, and credentials are never included/)).toBeTruthy();
  fireEvent.click(panel.getByRole("button", { name: "Retry vault sync" }));
  await panel.findByText(/Could not sync vault: Error: Permission denied/);
  fireEvent.click(panel.getByRole("button", { name: "Retry vault sync" }));
  await panel.findByText("Vault is ready. No conflicts reported.");
  fireEvent.click(panel.getByRole("button", { name: "Export public knowledge" }));
  await panel.findByText(/Could not export public knowledge: Error: Disk full/);
  fireEvent.click(panel.getByRole("button", { name: "Export public knowledge" }));
  await panel.findByText("/exports/public-fresh");
  expect(exportFolder).toHaveBeenCalledTimes(2);
  expect(rawExport).not.toHaveBeenCalled();
  expect(turns).not.toHaveBeenCalled();
  expect(page).not.toHaveBeenCalled();
  fireEvent.click(panel.getByRole("button", { name: "Close" }));
  fireEvent.click(screen.getByRole("button", { name: "Library" }));
  expect(screen.getByText("/exports/public-fresh")).toBeTruthy();
});

test("history loading locks switching and sending until the selected transcript arrives", async () => {
  const client = createFakeClient();
  const first = await client.sendMessage(null, "First saved question");
  const second = await client.sendMessage(null, "Second saved question");
  const saved = await client.listTurns(first.sessionId);
  let resolve!: (value: typeof saved) => void;
  const load = vi.spyOn(client, "listTurns").mockImplementation(() => new Promise((r) => { resolve = r; }));
  const send = vi.spyOn(client, "sendMessage");
  render(<App client={client} productName="Test" />);
  const history = screen.getByRole("combobox", { name: "Conversation history" }) as HTMLSelectElement;
  await waitFor(() => expect(history.options.length).toBe(3));
  fireEvent.change(history, { target: { value: first.sessionId } });
  expect(history.disabled).toBe(true);
  expect((screen.getByRole("button", { name: "New conversation" }) as HTMLButtonElement).disabled).toBe(true);
  // Also exercise the synchronous hook guard, even if a change bypasses disabled HTML.
  fireEvent.change(history, { target: { value: second.sessionId } });
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "Too soon" } });
  fireEvent.submit(screen.getByRole("textbox").closest("form")!);
  expect(send).not.toHaveBeenCalled();
  expect(load).toHaveBeenCalledTimes(1);
  await act(async () => resolve(saved));
  await screen.findByText("First saved question");
  expect(screen.queryByText("Second saved question")).toBeNull();
  expect(history.disabled).toBe(false);
});
