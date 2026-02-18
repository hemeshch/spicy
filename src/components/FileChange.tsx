import './FileChange.css';

const RAINBOW_COLORS = [
  'var(--rainbow-green)',
  'var(--rainbow-yellow)',
  'var(--rainbow-orange)',
  'var(--rainbow-red)',
  'var(--rainbow-purple)',
  'var(--rainbow-blue)',
];

interface FileChangeProps {
  index: number;
  change: {
    component?: string;
    filename: string;
    description: string;
  };
}

export function FileChange({ index, change }: FileChangeProps) {
  const color = RAINBOW_COLORS[index % RAINBOW_COLORS.length];

  return (
    <div className="file-change-chip">
      {change.component && (
        <>
          <span className="chip-component" style={{ color }}>{change.component}</span>
          <span className="chip-separator">&middot;</span>
        </>
      )}
      <span className="chip-text">{change.description}</span>
      <span className="chip-filename">{change.filename}</span>
    </div>
  );
}
