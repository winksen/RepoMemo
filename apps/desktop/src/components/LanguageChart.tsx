import type { RepoLanguageShare } from "../types";

const RADIUS = 52;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
/** A gap between slices, in circumference units, so neighbours stay distinguishable. */
const GAP = 1.5;

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

function formatShare(share: number) {
  if (share > 0 && share < 0.01) return "<1%";
  return `${Math.round(share * 100)}%`;
}

/** Languages by share of repository bytes: a donut with a legend that carries the exact figures. */
export function LanguageChart({ languages, totalBytes }: { languages: RepoLanguageShare[]; totalBytes: number }) {
  const total = totalBytes || languages.reduce((sum, share) => sum + share.bytes, 0);
  if (!languages.length || !total) return <p className="shared-muted-copy">No language data yet.</p>;

  let offset = 0;
  const slices = languages.map((share, index) => {
    const fraction = share.bytes / total;
    const length = Math.max(0, fraction * CIRCUMFERENCE - (languages.length > 1 ? GAP : 0));
    const slice = { share, fraction, color: share.language === "Other" ? "var(--color-text-subtle)" : `var(--chart-${(index % 7) + 1})`, length, offset };
    offset += fraction * CIRCUMFERENCE;
    return slice;
  });
  const top = slices[0];
  const description = slices.map(({ share, fraction }) => `${share.language} ${formatShare(fraction)}`).join(", ");

  return <figure className="rm-language-chart">
    <svg aria-label={`Languages by size: ${description}`} role="img" viewBox="0 0 140 140">
      <g fill="none" strokeWidth="18" transform="rotate(-90 70 70)">
        {slices.map(({ share, color, length, offset: start }) => <circle cx="70" cy="70" key={share.language} r={RADIUS} stroke={color} strokeDasharray={`${length} ${CIRCUMFERENCE - length}`} strokeDashoffset={-start}><title>{share.language}</title></circle>)}
      </g>
      <text className="rm-language-chart-value" textAnchor="middle" x="70" y="68">{formatShare(top.fraction)}</text>
      <text className="rm-language-chart-label" textAnchor="middle" x="70" y="85">{top.share.language}</text>
    </svg>
    <figcaption>
      <ul className="rm-language-legend">
        {slices.map(({ share, fraction, color }) => <li key={share.language}>
          <span aria-hidden="true" className="rm-language-swatch" style={{ background: color }} />
          <strong>{share.language}</strong>
          <span className="rm-language-meta">{share.files.toLocaleString()} {share.files === 1 ? "file" : "files"} · {formatBytes(share.bytes)}</span>
          <span className="rm-language-share">{formatShare(fraction)}</span>
        </li>)}
      </ul>
    </figcaption>
  </figure>;
}
