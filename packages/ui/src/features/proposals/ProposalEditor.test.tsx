import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "../../App";
import { createFakeClient } from "../../testing/fake-client";

afterEach(cleanup);

async function seed() {
  const client = createFakeClient();
  await client.sendMessage(null, "What is a VAE?");
  const original = (await client.pendingProposals())[0]!;
  await client.editProposal(original.id, {
    expectedRevision: original.revision, summary: original.summary,
    changes: [...original.changes, { op: "revise_article", concept: "variational-autoencoder", summary: "Original article summary", bodyMd: "Original article text" }],
  });
  return client;
}
async function openEditor() {
  fireEvent.click(screen.getByRole("button", { name: /Proposed updates/ }));
  fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
}

test("edit failure retains the draft; saving requires a separate approval of the revised proposal", async () => {
  const client = await seed();
  const edit = vi.spyOn(client, "editProposal");
  const apply = edit.getMockImplementation()!;
  edit.mockRejectedValueOnce(new Error("Database busy")).mockImplementation(apply);
  const approve = vi.spyOn(client, "approveProposal");
  render(<App client={client} productName="Test" />);
  await openEditor();
  expect(screen.queryByRole("button", { name: "Approve" })).toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "Article text (Markdown)" }), { target: { value: "My corrected explanation" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Update summary" }), { target: { value: "Corrected VAE explanation" } });
  fireEvent.click(screen.getByRole("button", { name: "Remove change 2" }));
  fireEvent.click(screen.getByRole("button", { name: "Save edits" }));
  await screen.findByText(/Could not save edits.*Database busy/);
  expect((screen.getByRole("textbox", { name: "Article text (Markdown)" }) as HTMLTextAreaElement).value).toBe("My corrected explanation");
  fireEvent.click(screen.getByRole("button", { name: "Save edits" }));
  await screen.findByRole("button", { name: "Approve" });
  expect(await client.listConcepts()).toHaveLength(0);
  expect(approve).not.toHaveBeenCalled();
  const p = (await client.pendingProposals())[0]!;
  expect(p.revision).toBe(2);
  expect(p.changes.some((c) => c.op === "add_observation")).toBe(false);
  expect(screen.getByText("My corrected explanation")).toBeTruthy();
  const historySummary = screen.getByText(/Edit history · 2 edits/);
  fireEvent.click(historySummary);
  const history = historySummary.closest("details")!;
  history.open = true;
  fireEvent(history, new Event("toggle"));
  await screen.findByText(/Original proposal · Add Variational Autoencoder/);
  fireEvent.click(screen.getByRole("button", { name: "Approve" }));
  await waitFor(() => expect(approve).toHaveBeenCalledExactlyOnceWith(p.id, p.revision));
});

test("concurrent edits preserve the local draft and block stale saving until the latest version is reviewed", async () => {
  const client = await seed();
  render(<App client={client} productName="Test" />);
  await openEditor();
  fireEvent.change(screen.getByRole("textbox", { name: "Update summary" }), { target: { value: "Unsaved local wording" } });
  const old = (await client.pendingProposals())[0]!;
  await act(async () => { await client.editProposal(old.id, { expectedRevision: old.revision, summary: "Changed elsewhere", changes: old.changes }); });
  await screen.findByText(/This proposal changed while you were editing/);
  expect((screen.getByRole("textbox", { name: "Update summary" }) as HTMLInputElement).value).toBe("Unsaved local wording");
  expect((screen.getByRole("button", { name: "Save edits" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel editing" }));
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect((screen.getByRole("textbox", { name: "Update summary" }) as HTMLInputElement).value).toBe("Changed elsewhere");
});

test("removing all changes prevents saving and cancellation restores the proposal", async () => {
  const client = await seed();
  const edit = vi.spyOn(client, "editProposal");
  render(<App client={client} productName="Test" />);
  await openEditor();
  for (let i = 3; i > 0; i--) fireEvent.click(screen.getByRole("button", { name: `Remove change ${i}` }));
  expect((screen.getByRole("button", { name: "Save edits" }) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText(/Keep at least one change/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel editing" }));
  expect(edit).not.toHaveBeenCalled();
  const drawer = within(screen.getByRole("complementary"));
  expect(drawer.getByRole("button", { name: "Approve" })).toBeTruthy();
  expect(drawer.getByText("Original article text")).toBeTruthy();
});

test("unsaved edits survive closing and switching inspectors, and cancel clears the retained draft", async () => {
  const client = await seed();
  render(<App client={client} productName="Test" />);
  await openEditor();
  fireEvent.change(screen.getByRole("textbox", { name: "Update summary" }), { target: { value: "Keep this draft" } });
  fireEvent.keyDown(document, { key: "Escape" });
  fireEvent.click(screen.getByRole("button", { name: /Models/ }));
  fireEvent.click(screen.getByRole("button", { name: /Proposed updates/ }));
  expect((await screen.findByRole("textbox", { name: "Update summary" }) as HTMLInputElement).value).toBe("Keep this draft");
  fireEvent.click(screen.getByRole("button", { name: "Cancel editing" }));
  fireEvent.keyDown(document, { key: "Escape" });
  await openEditor();
  expect((screen.getByRole("textbox", { name: "Update summary" }) as HTMLInputElement).value).not.toBe("Keep this draft");
});
