import type { Welcome } from "../../types";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";

/** The questions as one per line, and back: blanks and repeats are dropped, and the most is six. */
export function questionsFromText(text: string): string[] {
  return text.split(String.fromCharCode(10)).map((line) => line.trim()).filter((line, index, all) => line && all.indexOf(line) === index).slice(0, 6);
}

/** A welcome as it is kept: trimmed, with no blank or repeated question. */
export function cleanWelcome(value: Welcome | undefined): Welcome {
  return { greeting: (value?.greeting ?? "").trim(), questions: questionsFromText((value?.questions ?? []).join(String.fromCharCode(10))) };
}

/** Whether two welcomes say the same, for a form's dirty check. */
export function sameWelcome(left: Welcome | undefined, right: Welcome | undefined): boolean {
  return (left?.greeting ?? "").trim() === (right?.greeting ?? "").trim() && JSON.stringify(left?.questions ?? []) === JSON.stringify(right?.questions ?? []);
}

/**
 * CAIRN's greeting and starter questions (V0.9.116). Written here, they replace the standard greeting and the template's suggested
 * questions in the chat. A workspace's own replace its organization's. Empty keeps what there is.
 */
export function WelcomeFields({ disabled, onChange, value }: { disabled?: boolean; onChange: (next: Welcome) => void; value: Welcome }) {
  return <>
    <Field hint="One line where CAIRN says hello, at most 120 characters. Empty keeps the standard one." label="Greeting"><Input disabled={disabled} maxLength={120} onChange={(event) => onChange({ ...value, greeting: event.target.value })} value={value.greeting} /></Field>
    <Field hint="One question per line, up to six, each at most 140 characters. They replace the template's suggested questions." label="Starter questions"><Textarea disabled={disabled} onChange={(event) => onChange({ ...value, questions: event.target.value.split(String.fromCharCode(10)) })} rows={4} value={value.questions.join(String.fromCharCode(10))} /></Field>
  </>;
}
