import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("renders the music surfaces and speaker controls", () => {
    render(<Sidebar active={null} onNavigate={vi.fn()} />);
    expect(screen.getByText("Library")).toBeInTheDocument();
    expect(screen.getByText("Lists")).toBeInTheDocument();
    expect(screen.getByText("Queue")).toBeInTheDocument();
    expect(screen.getByText("Speakers")).toBeInTheDocument();
    expect(screen.getByText("EQ")).toBeInTheDocument();
  });

  it("highlights only the active navigation item", () => {
    render(<Sidebar active="queue" onNavigate={vi.fn()} />);
    expect(screen.getByText("Queue").closest("button")!.className).toContain("color-accent");
    expect(screen.getByText("Library").closest("button")!.className).toContain(
      "color-text-secondary"
    );
  });

  it("navigates directly to speaker output controls", () => {
    const onNavigate = vi.fn();
    render(<Sidebar active={null} onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText("Speakers"));
    expect(onNavigate).toHaveBeenCalledWith("devices");
  });

  it("does not render a selected-device shortcut", () => {
    render(<Sidebar active={null} onNavigate={vi.fn()} />);
    expect(screen.queryByText("Kitchen")).toBeNull();
  });
});
