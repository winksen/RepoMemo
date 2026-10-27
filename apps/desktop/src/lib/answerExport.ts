import type { AskAnswer, Citation } from "../types";

export interface ExportCitationSource {
  sourceId: string;
  reviewStatus: string;
  headingPath: string | null;
}

export async function answerExportMarkdown(
  question: string,
  answer: AskAnswer,
  resolve: (citation: Citation) => Promise<ExportCitationSource>,
): Promise<string> {
  const lines = [`# ${question}`, "", answer.answer_markdown.trim(), "", "## Sources", ""];
  if (answer.citations.length === 0) lines.push("_No sources cited._");
  for (const [index, citation] of answer.citations.entries()) {
    try {
      const source = await resolve(citation);
      const location = citation.start_line === null
        ? source.headingPath ? `section ${source.headingPath}` : "whole artifact"
        : citation.end_line !== null && citation.end_line !== citation.start_line
          ? `lines ${citation.start_line}-${citation.end_line}` : `line ${citation.start_line}`;
      lines.push(`${index + 1}. [${citation.title}](${citation.path}) — artifact \`${citation.artifact_id}\`; source \`${source.sourceId}\`; ${location}; review: ${source.reviewStatus}`);
    } catch {
      lines.push(`${index + 1}. MISSING SOURCE — artifact \`${citation.artifact_id}\`${citation.chunk_id ? `, chunk \`${citation.chunk_id}\`` : ""}; source is unavailable or access was revoked.`);
    }
  }
  return `${lines.join("\n")}\n`;
}
