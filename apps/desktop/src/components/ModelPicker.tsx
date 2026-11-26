import type { OfferedModel } from "../types";
import { Dropdown } from "./ui/dropdown";

/** What a model is called in the picker: its name, where it runs and, when it takes tools, that CAIRN can work with it as an agent. */
export function modelLabel(model: OfferedModel): string {
  return `${model.name} · ${model.locality === "cloud" ? "Cloud" : "Local"}${model.agent ? " · Agent" : ""}`;
}

/**
 * The model line under the message box (V0.9.86): a picker of the models offered in the workspace. The choice belongs to the chat and
 * is kept for its next messages; without one the model that answers by default is shown.
 */
export function ModelPicker({ models, onChange, selected }: { models: OfferedModel[]; onChange: (id: string) => void; selected: string | null }) {
  const current = models.find((model) => model.id === selected) ?? models.find((model) => model.default) ?? models[0];
  return <Dropdown aria-label="Model for this chat" className="shared-model-picker" onValueChange={onChange} options={models.map((model) => ({ label: modelLabel(model), value: model.id }))} value={current?.id} />;
}
