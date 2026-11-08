import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { Markdown, wikiLinks } from "./Markdown";

afterEach(cleanup);

test("wiki links, no raw HTML, no navigating links", () => {
  expect(wikiLinks("see [[latent-space]] and [[vae|the VAE]]")).toBe(
    "see [latent space](wiki:latent-space) and [the VAE](wiki:vae)",
  );
  const onLink = vi.fn();
  const { container } = render(
    <Markdown
      source={"## Intro\n\nUses [[latent-space]]. <script>alert(1)</script> [site](https://example.com)"}
      onLink={onLink}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "latent space" }));
  expect(onLink).toHaveBeenCalledWith("latent-space");
  expect(container.querySelector("script")).toBeNull();
  expect(container.querySelector("a")).toBeNull();
  expect(screen.getByRole("heading", { name: "Intro" })).toBeTruthy();
});
