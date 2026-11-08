import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ClientProvider } from "../../client-context";
import { createFakeClient } from "../../testing/fake-client";
import { Conversation } from "./Conversation";
import type { ConversationState } from "./useConversation";

afterEach(cleanup);
function mount(send: ConversationState["send"]) {
  return render(<ClientProvider value={createFakeClient()}><QueryClientProvider client={new QueryClient()}><Conversation state={{ messages: [], sessionId: null, busy: false, loadingHistory: false, historyError: null, resume: async () => {}, newConversation: () => {}, send, onEvent: () => {} }} /></QueryClientProvider></ClientProvider>);
}

test("demo shortcut prepares a question without sending it", async () => {
  const send = vi.fn();
  mount(send);
  await screen.findByText("Offline demo");
  fireEvent.click(screen.getByRole("button", { name: /What is a VAE/ }));
  const input = screen.getByRole("textbox", { name: "Your next question" }) as HTMLTextAreaElement;
  expect(input.value).toBe("What is a VAE?");
  expect(document.activeElement).toBe(input);
  expect(send).not.toHaveBeenCalled();
});

test("submission is locked until acknowledged and preserves the draft after failure", async () => {
  let reject!: (reason: Error) => void;
  const send = vi.fn(() => new Promise<void>((_, r) => { reject = r; }));
  mount(send);
  const input = screen.getByRole("textbox", { name: "Your next question" }) as HTMLTextAreaElement;
  fireEvent.change(input, { target: { value: "Explain this" } });
  fireEvent.submit(input.closest("form")!);
  fireEvent.submit(input.closest("form")!);
  expect(send).toHaveBeenCalledTimes(1);
  reject(new Error("Provider unavailable"));
  expect((await screen.findByRole("alert")).textContent).toContain("Provider unavailable");
  expect(input.value).toBe("Explain this");
  await waitFor(() => expect(input.readOnly).toBe(false));
  expect(document.activeElement).toBe(input);
});

test("Shift Enter and IME composition do not send; Enter sends once", async () => {
  const send = vi.fn(async () => {});
  mount(send);
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "A question" } });
  fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  fireEvent.keyDown(input, { key: "Enter", isComposing: true });
  expect(send).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(send).toHaveBeenCalledExactlyOnceWith("A question"));
});
