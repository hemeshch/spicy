import './FileTabs.css';

const RAINBOW_COLORS = [
  'var(--rainbow-green)',
  'var(--rainbow-yellow)',
  'var(--rainbow-orange)',
  'var(--rainbow-red)',
  'var(--rainbow-purple)',
  'var(--rainbow-blue)',
];

interface FileTabsProps {
  files: string[];
  activeIndex: number;
  onSelect: (index: number) => void;
}

export function FileTabs({ files, activeIndex, onSelect }: FileTabsProps) {
  if (files.length === 0) return null;

  return (
    <div className="file-tabs">
      {files.map((file, i) => {
        const color = RAINBOW_COLORS[i % RAINBOW_COLORS.length];
        const isActive = i === activeIndex;

        return (
          <button
            key={file}
            className={`file-tab ${isActive ? 'active' : ''}`}
            style={{
              '--tab-color': color,
            } as React.CSSProperties}
            onClick={() => onSelect(i)}
          >
            <span className="file-tab-name">{file}</span>
            {isActive && <div className="file-tab-indicator" />}
          </button>
        );
      })}
    </div>
  );
}

export function getTabColor(index: number): string {
  return RAINBOW_COLORS[index % RAINBOW_COLORS.length];
}
