import { useEffect, useRef, useState } from "react";
import { onAssistantCommand, takeAssistantCommand, type AssistantCommand } from "./assistantBridge";

/**
 * Commands from the command palette wait here until the assistant `ready` (its capabilities are loaded
 * and it is idle), then `handle` receives each one, once.
 */
export function useAssistantCommands({ handle, ready }: { handle: (command: AssistantCommand) => void; ready: boolean }) {
  const [queued, setQueued] = useState<AssistantCommand | null>(null);
  const latest = useRef(handle);
  latest.current = handle;
  useEffect(() => {
    const take = () => { const command = takeAssistantCommand(); if (command) setQueued(command); };
    take();
    return onAssistantCommand(take);
  }, []);
  useEffect(() => {
    if (!queued || !ready) return;
    setQueued(null);
    latest.current(queued);
  }, [queued, ready]);
}
