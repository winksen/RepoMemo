import * as PopoverPrimitive from "@radix-ui/react-popover";
import { ChatCircleText, FileText, Lightning, PencilSimple, SlidersHorizontal, Trash } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import type { AgentCapabilities, AgentCapability, AgentConversation, AgentReply } from "../types";
import { formatRecentTime } from "../web/format";
import { Button } from "./ui/button";
import { ICON_SIZE } from "./ui/icon";
import { IconButton } from "./ui/icon-button";

/** What a chat has used so far, worked out from the replies on screen. */
export interface ChatFacts {
  exchanges: number;
  files: { id: string; title: string }[];
  capabilities: { id: AgentCapability; count: number }[];
  generated: number;
  replies: number;
}

export function chatFacts(replies: (AgentReply | undefined)[]): ChatFacts {
  const files = new Map<string, string>();
  const used = new Map<AgentCapability, number>();
  let generated = 0;
  let answered = 0;
  for (const reply of replies) {
    if (!reply) continue;
    answered += 1;
    if (reply.generated) generated += 1;
    if (reply.capability) used.set(reply.capability, (used.get(reply.capability) ?? 0) + 1);
    for (const citation of reply.citations) if (!files.has(citation.artifact_id)) files.set(citation.artifact_id, citation.title || citation.path);
    for (const file of reply.files) if (!files.has(file.id)) files.set(file.id, file.title || file.id);
  }
  return {
    exchanges: replies.length,
    files: [...files].map(([id, title]) => ({ id, title })),
    capabilities: [...used].map(([id, count]) => ({ id, count })),
    generated,
    replies: answered,
  };
}

/**
 * The options of the chat on Cairn's page, a dropdown at the top right of the chat: how many exchanges it holds, the files it used
 * (each opens), what it asked of CAIRN and how many replies an AI provider wrote, and rename and delete.
 */
export function ChatOptions({ capabilities, conversation, onDelete, onOpenArtifact, onRename, replies }: {
  capabilities: AgentCapabilities | null;
  conversation: AgentConversation | null;
  onDelete: () => void;
  onOpenArtifact: (artifactId: string) => void;
  onRename: () => void;
  replies: (AgentReply | undefined)[];
}) {
  const [open, setOpen] = useState(false);
  const facts = useMemo(() => chatFacts(replies), [replies]);
  const labelOf = (id: AgentCapability) => capabilities?.capabilities.find((capability) => capability.id === id)?.label ?? id.replace(/_/g, " ");
  return (
    <PopoverPrimitive.Root onOpenChange={setOpen} open={open}>
      <PopoverPrimitive.Trigger asChild>
        <IconButton appearance="plain" className="shared-chat-options-trigger" label="Chat options" size="sm"><SlidersHorizontal size={ICON_SIZE.lg} /></IconButton>
      </PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content align="end" aria-label="Chat options" className="shared-chat-options" collisionPadding={12} sideOffset={8}>
          <dl className="shared-chat-options-facts">
            <div><dt><ChatCircleText size={ICON_SIZE.md} /> Exchanges</dt><dd>{facts.exchanges} <small>({facts.exchanges * 2} messages back and forth)</small></dd></div>
            {conversation ? <div><dt>Started</dt><dd>{formatRecentTime(conversation.created_at)}</dd></div> : null}
            {conversation ? <div><dt>Last activity</dt><dd>{formatRecentTime(conversation.updated_at)}</dd></div> : null}
            <div><dt><Lightning size={ICON_SIZE.md} /> Written by AI</dt><dd>{facts.generated} of {facts.replies} {facts.replies === 1 ? "reply" : "replies"}{capabilities?.provider_name ? <small> · {capabilities.provider_name}</small> : null}</dd></div>
          </dl>
          {facts.capabilities.length ? <div className="shared-chat-options-block"><strong>Asked of CAIRN</strong><ul>{facts.capabilities.map((entry) => <li key={entry.id}><span>{labelOf(entry.id)}</span><small>{entry.count}×</small></li>)}</ul></div> : null}
          <div className="shared-chat-options-block">
            <strong><FileText size={ICON_SIZE.md} /> Files used ({facts.files.length})</strong>
            {facts.files.length
              ? <ul>{facts.files.map((file) => <li key={file.id}><button onClick={() => { setOpen(false); onOpenArtifact(file.id); }} type="button">{file.title}</button></li>)}</ul>
              : <p>No file has been used yet in this chat.</p>}
          </div>
          {conversation ? <div className="shared-chat-options-actions">
            <Button onClick={() => { setOpen(false); onRename(); }} type="button" variant="secondary"><PencilSimple size={ICON_SIZE.md} /> Rename</Button>
            <Button onClick={() => { setOpen(false); onDelete(); }} type="button" variant="secondary"><Trash size={ICON_SIZE.md} /> Delete</Button>
          </div> : null}
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
