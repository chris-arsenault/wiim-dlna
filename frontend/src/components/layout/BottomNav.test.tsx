import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { BottomNav } from "./BottomNav";

describe("BottomNav", () => {
  it("renders all tabs", () => {
    render(<BottomNav active="library" onNavigate={vi.fn()} />);
    expect(screen.getByText("Library")).toBeInTheDocument();
    expect(screen.getByText("Queue")).toBeInTheDocument();
    expect(screen.getByText("Speakers")).toBeInTheDocument();
    expect(screen.getByText("EQ")).toBeInTheDocument();
  });

  it("calls onNavigate with tab id on click", () => {
    const onNavigate = vi.fn();
    render(<BottomNav active="library" onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText("Queue"));
    expect(onNavigate).toHaveBeenCalledWith("queue");
    fireEvent.click(screen.getByText("Speakers"));
    expect(onNavigate).toHaveBeenCalledWith("devices");
  });

  it("highlights active tab with accent color class", () => {
    render(<BottomNav active="queue" onNavigate={vi.fn()} />);
    const queueBtn = screen.getByText("Queue").closest("button")!;
    const libraryBtn = screen.getByText("Library").closest("button")!;
    expect(queueBtn.className).toContain("color-accent");
    expect(libraryBtn.className).toContain("color-text-secondary");
  });
});
