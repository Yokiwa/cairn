import { screen } from "@testing-library/react";
import { renderApp } from "../test-utils";
import Sidebar from "./Sidebar";

test("Sidebar marks active route with aria-current", () => {
  renderApp(<Sidebar />, { route: "/instances/abc" });
  expect(screen.getByRole("link", { name: "Instances" })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("link", { name: "Home" })).not.toHaveAttribute("aria-current");
  expect(screen.getByRole("link", { name: "Settings" })).toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "Discover" })).not.toBeInTheDocument();
});
