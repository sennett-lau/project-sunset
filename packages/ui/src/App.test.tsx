import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";
import { createFakeClient } from "./testing/fake-client";

afterEach(cleanup);

test("ask → streamed answer → proposal badge → approve shows the wiki → undo hides it", async () => {
  render(<App client={createFakeClient()} productName="Test App" />);
  expect(screen.getByRole("heading", { name: "Test App" })).toBeTruthy();

  fireEvent.change(screen.getByPlaceholderText("Type a question…"), { target: { value: "What is a VAE?" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  await screen.findByText("A VAE encodes data as a distribution.");
  const badge = screen.getByRole("button", { name: /Proposed updates/ });
  await waitFor(() => expect(badge.textContent).toContain("1"));

  fireEvent.click(badge);
  const drawer = screen.getByRole("complementary");
  fireEvent.click(await within(drawer).findByRole("button", { name: "Approve" }));

  // Wiki: article title, private overlay, and the concept_flow visual.
  await screen.findByRole("heading", { name: "Variational Autoencoder" });
  expect(await screen.findByText("Your learning notes")).toBeTruthy();
  expect(screen.getByText("Encoder")).toBeTruthy();
  await waitFor(() => expect(badge.textContent).toContain("0"));

  fireEvent.click(await within(drawer).findByRole("button", { name: "Undo" }));
  await screen.findByText(/Your wiki is empty/);
});
