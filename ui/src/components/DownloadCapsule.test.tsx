import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderApp } from "../test-utils";
import { emitMockEvent } from "../lib/mockBackend";
import DownloadCapsule from "./DownloadCapsule";

test("DownloadCapsule shows percent, speed and all-done", async () => {
  renderApp(<DownloadCapsule />);
  expect(screen.queryByRole("button", { name: /downloads/i })).not.toBeInTheDocument();
  act(() =>
    emitMockEvent({
      type: "progress",
      id: "t1",
      progress: { stage: "assets", done_files: 5, total_files: 10, done_bytes: 50, total_bytes: 100, bytes_per_sec: 2 * 1048576 },
    }),
  );
  const button = await screen.findByRole("button", { name: /downloads/i });
  expect(button).toHaveTextContent("50%");
  expect(button).toHaveTextContent("2.0 MB/s");
  await userEvent.click(button);
  expect(screen.getByText(/assets/i)).toBeInTheDocument();
  act(() => emitMockEvent({ type: "done", id: "t1", result: null }));
  expect(await screen.findByText("All done")).toBeInTheDocument();
});

test("a running game whose files are verified shows all done, not a spinner", async () => {
  renderApp(<DownloadCapsule />);
  act(() =>
    emitMockEvent({
      type: "progress",
      id: "launch1",
      progress: { stage: "game", done_files: 300, total_files: 300, done_bytes: 0, total_bytes: 0, bytes_per_sec: 0 },
    }),
  );
  // The launch task keeps running while the game is open, but nothing is downloading.
  expect(await screen.findByText("All done")).toBeInTheDocument();
});
