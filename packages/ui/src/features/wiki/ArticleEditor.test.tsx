import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "../../App";
import { createFakeClient } from "../../testing/fake-client";

afterEach(cleanup);
async function seed() {
  const client = createFakeClient();
  await client.sendMessage(null, "What is a VAE?");
  const proposal = (await client.pendingProposals())[0]!;
  await client.approveProposal(proposal.id, proposal.revision);
  return client;
}

test("article edits remain proposals until approval, can open from review and undo", async () => {
  const client = await seed();
  const concept = (await client.listConcepts())[0]!;
  const original = (await client.conceptPage(concept.id)).article!;
  await client.setSaveMode("auto");
  render(<App client={client} productName="Test" />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit article" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Article body (Markdown)" }), { target: { value: "My carefully revised article." } });
  fireEvent.click(screen.getByText("Preview article"));
  expect(screen.getByText("My carefully revised article.", { selector: "p" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Review article edit" }));
  await screen.findByText(/Article edit proposed/);
  expect((await client.conceptPage(concept.id)).article?.id).toBe(original.id);
  fireEvent.click(screen.getByRole("button", { name: /Proposed updates/ }));
  const drawer = screen.getByRole("complementary");
  expect(await within(drawer).findByText(/Your article edit/)).toBeTruthy();
  fireEvent.click(within(drawer).getByRole("button", { name: "Edit" }));
  expect(within(drawer).queryByRole("button", { name: "Remove change 1" })).toBeNull();
  expect((within(drawer).getByRole("textbox", { name: "Concept address" }) as HTMLInputElement).disabled).toBe(true);
  fireEvent.click(within(drawer).getByRole("button", { name: "Cancel editing" }));
  fireEvent.click(within(drawer).getByRole("button", { name: "Approve" }));
  await waitFor(async () => expect((await client.conceptPage(concept.id)).article?.bodyMd).toBe("My carefully revised article."));
  const open = await within(drawer).findAllByRole("button", { name: "Open" });
  fireEvent.click(open[0]!);
  await screen.findByRole("button", { name: "Edit article" });
  const edit = (await client.recentProposals(1))[0]!;
  await act(async () => { await client.undoProposal(edit.id); });
  await waitFor(async () => expect((await client.conceptPage(concept.id)).article?.id).toBe(original.id));
});

test("article draft survives Wiki unmount and failed submission, and discard clears it", async () => {
  const client = await seed();
  vi.spyOn(client, "proposeArticleEdit").mockRejectedValue(new Error("article changed since editing"));
  const view = render(<App client={client} productName="Test" />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit article" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Article summary" }), { target: { value: "Unsaved wording" } });
  view.unmount();
  render(<App client={client} productName="Test" />);
  expect((await screen.findByRole("textbox", { name: "Article summary" }) as HTMLTextAreaElement).value).toBe("Unsaved wording");
  fireEvent.click(screen.getByRole("button", { name: "Review article edit" }));
  await screen.findByText(/Could not propose this edit.*article changed/);
  expect((screen.getByRole("textbox", { name: "Article summary" }) as HTMLTextAreaElement).value).toBe("Unsaved wording");
  fireEvent.click(screen.getByRole("button", { name: "Discard article draft" }));
  fireEvent.click(screen.getByRole("button", { name: "Edit article" }));
  expect((screen.getByRole("textbox", { name: "Article summary" }) as HTMLTextAreaElement).value).not.toBe("Unsaved wording");
});

test("a newer saved revision blocks the old draft without discarding its text", async () => {
  const client = await seed();
  const concept = (await client.listConcepts())[0]!;
  const original = (await client.conceptPage(concept.id)).article!;
  render(<App client={client} productName="Test" />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit article" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Article summary" }), { target: { value: "My unsaved local edit" } });
  await act(async () => {
    const other = await client.proposeArticleEdit(concept.id, original.id, "Newer definition", "Newer saved body");
    await client.approveProposal(other.id, other.revision);
  });
  await screen.findByText(/This article changed while you were editing/);
  expect((screen.getByRole("textbox", { name: "Article summary" }) as HTMLTextAreaElement).value).toBe("My unsaved local edit");
  expect((screen.getByRole("button", { name: "Review article edit" }) as HTMLButtonElement).disabled).toBe(true);
});
