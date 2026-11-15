/**
 * The message catalogue (V0.6.27): the words of the core journeys (the first-value checklist, My work and the reviewer panel)
 * live here, under stable keys, so a language can be added without searching the components. English is the only catalogue
 * that ships. `setMessages` lets a test, or a future language, replace some or all of it; a key a replacement leaves out falls
 * back to English, so a partial language never shows a blank.
 *
 * A message may name values in braces, such as `{done}`; `t` fills them in.
 */
const en = {
  "onboarding.title": "Getting started with {template}",
  "onboarding.progress": "{done} of {total} done",
  "onboarding.stepDone": "Done",
  "onboarding.stepTodo": "To do",
  "onboarding.hide": "Hide for now",
  "onboarding.hideComplete": "Hide, all done",
  "onboarding.show": "Show the getting-started checklist ({done} of {total})",
  "onboarding.readFailed": "The checklist could not be read.",
  "onboarding.saveFailed": "That could not be saved.",
  "myWork.title": "My work",
  "myWork.waiting": "{total} waiting",
  "myWork.waitingOverdue": "{total} waiting, {overdue} overdue",
  "myWork.empty": "Nothing is waiting for you. Tasks assigned to you, files you own that need review and new mentions appear here.",
  "myWork.readFailed": "Your work could not be read.",
  "myWork.task": "Task · {priority} priority",
  "myWork.taskDue": "Task · {priority} priority · due {due}",
  "myWork.review": "Review · {workspace}",
  "myWork.reviewReason": "Review · {workspace} · {reason}",
  "myWork.mention": "Mention",
  "myWork.mentionBody": "Mention · {body}",
  "reviewer.title": "Reviewer",
  "reviewer.overdue": "Review is overdue",
  "reviewer.next": "Next review {date}",
  "reviewer.noDate": "No review date",
  "reviewer.nobody": "Nobody is accountable for reviewing this yet.",
  "reviewer.accountable": "is accountable for checking this.",
  "reviewer.left": "has left this workspace, so nobody is accountable. Choose someone else.",
  "reviewer.lastReviewed": "Last reviewed {date}.",
} as const;

export type MessageKey = keyof typeof en;
export type Catalogue = Partial<Record<MessageKey, string>>;

let replacement: Catalogue = {};

/** Replaces messages (all or some); `null` goes back to English. */
export function setMessages(catalogue: Catalogue | null) {
  replacement = catalogue ?? {};
}

/** The message for `key`, with `{name}` values filled in. */
export function t(key: MessageKey, values: Record<string, string | number> = {}): string {
  const template = replacement[key] ?? en[key];
  return template.replace(/\{(\w+)\}/g, (match, name: string) => (name in values ? String(values[name]) : match));
}

/** Every key the English catalogue has, for a test or a translator's checklist. */
export const messageKeys = Object.keys(en) as MessageKey[];
